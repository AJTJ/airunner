//! `air selftest`: red/green probes for every check, run against an in-memory ledger and a
//! scratch git repo. A check that matches nothing prints RED (corpus: guards that pass on
//! nothing are the anti-pattern). Exit 1 if any probe fails.
//!
//! **Writing a probe (air-jc0): never hold a second copy of a number some rule owns.** The
//! dangerous literal is the one only ONE side of the assertion knows about; a fixture whose
//! expectation is computed from itself cannot rot. So derive the fixture from the threshold
//! (`Thresholds::default().idle_with_claim_min`, `attribution::cutoff()`, `install::SURFACE`)
//! rather than
//! writing a number beside it, and put the value in the probe's name so a changed rule RENAMES
//! the probe instead of breaking it. Two controls before you believe a probe: neutralise the rule
//! and see it go red on a mutant that COMPILES, then change the rule's number and see it stay
//! green. A copied number passes the first and fails the second, which is the adopter's.
//! The worst case is the number that moves on its own: a hard-coded date against fixtures built
//! from the clock left main red for six days (air-24e).

use std::path::Path;
use std::process::Command;

use air_hooks::{GateFacts, handover_verdict};
use air_ledger::Ledger;
use air_ledger::verify::{Kind, VerifyRun, new_id};
use serde::Serialize;
use serde_json::Value;

use crate::cmd::emit;
use crate::cmd::hook::{handover_gate, is_handover_command};

/// The ONE way a probe spawns `air` (air-dws). Identity comes from the launcher's environment
/// since air-75u (`AIR_ROLE`, `BEADS_ACTOR`, `AIR_PROJECT`, `AIR_ENFORCE`), so a child that
/// inherits the shell's copy answers for whoever is running the suite: the SubagentStop probe
/// passed in every worker's worktree and failed in the coordinator's shell, which is where
/// `make release` runs, and no v0.2.10 could be cut. Every probe that wants an identity sets
/// it on the returned `Command` explicitly; none inherits one.
///
/// [`probe_every_air_spawn_pins_identity`] reads this file and fails if a raw
/// `Command::new(exe)` appears anywhere else, so the class does not come back one probe at a
/// time.
fn air_command(exe: &Path, cwd: &Path) -> Command {
    let mut c = Command::new(exe);
    c.current_dir(cwd)
        .env_remove("AIR_ROLE")
        .env_remove("BEADS_ACTOR")
        .env_remove("AIR_PROJECT")
        .env_remove("AIR_ENFORCE");
    c
}

/// How many raw `air` spawns a probe file holds beside the helper: lines building a
/// `Command::new` on the current executable. Pure over the text so the probe can show a
/// violation as well as the absence of one.
fn raw_air_spawns(source: &str) -> usize {
    source
        .lines()
        .map(str::trim_start)
        .filter(|l| l.starts_with("let ") || l.starts_with("Command::new"))
        .filter(|l| l.contains("Command::new(exe)") || l.contains("Command::new(&exe)"))
        .count()
}

/// air-682: the edit that neutralises the rule a probe names, declared next to the probe so it
/// can be RUN. The adopter's standard, adopted over ours by owner ruling: a probe is evidence only
/// once it has been seen failing with its rule neutralised, and the evidence is a revert, not an
/// intention. `air selftest` claiming "a probe that matches nothing prints red" is weaker,
/// because a vacuous probe also prints red for reasons of its own.
///
/// Three ways a revert demonstration misleads, all three of which `prove` reports separately:
///
/// 1. **A mutant that does not build.** the adopter's first run scored 15 of 15 red; two were a
///    syntax error, so the guard crashed and both probes went red for nothing. A mutation that
///    fails to compile is reported BROKEN and never counted as evidence.
/// 2. **A blanket mutant** (always-allow, always-deny) shows a probe is wired to the guard at
///    all, not that it tests the right rule; a vacuous probe behaves exactly like a real one
///    under it. So `from` names one branch, never a whole function or a top-level flag.
/// 3. **A mutation that reaches the WRONG PATH.** Theirs hit an exception branch while the case
///    under test reached a parse branch two lines below. It compiled, ran, neutralised nothing,
///    and the probe was reported vacuous. "That is the costliest of the three, because the
///    response to it is to go and fix a good probe." The guard against it is `also_red`: naming
///    every probe expected to fall with this rule forces the author to know what the mutation
///    actually reaches, and an unexpected survivor or casualty is reported rather than averaged
///    away.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Mutation {
    /// Repo-relative path of the file that OWNS the rule.
    pub file: &'static str,
    /// Exact text to replace. Must occur exactly once, or the mutation is BROKEN: an ambiguous
    /// or missing anchor is the wrong-path failure waiting to happen.
    pub from: &'static str,
    /// The neutralised form.
    pub to: &'static str,
    /// Other probes that legitimately share this rule and are expected to go red with it. Every
    /// probe not named here must stay GREEN, which is the assertion that separates a mutation
    /// reaching one branch from one that took out the whole guard.
    pub also_red: &'static [&'static str],
}

#[derive(Debug, Serialize)]
pub struct Probe {
    pub name: &'static str,
    pub red_fires: bool,
    pub green_passes: bool,
}

/// Why a probe could not run, when it could not (air-g7e).
///
/// Twenty-eight probes spawn a child, poll for a file, or open a scratch repo, and every one
/// of them ended `.unwrap_or_else(blocked)` — so the error text saying WHAT went wrong was
/// built and thrown away. A busy machine and a broken mechanism both rendered as
/// `red SILENT / green BLOCKED`, which is what the coordinator saw four times at load 100-186
/// and could do nothing with except run it again.
///
/// `air selftest` prints these under the probe lines. Deliberately NOT attributed to a probe
/// by name: the reasons arrive in evaluation order and a probe can fail inside a nested
/// closure, so an attribution would be a guess printed as a fact. The text names its own
/// fixture.
///
/// Removal: when no probe needs a spawned child to make its point.
fn blocked_reasons() -> &'static std::sync::Mutex<Vec<String>> {
    static R: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();
    R.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Record why a probe could not run and answer "neither half held" — which is what every one
/// of these sites already answered, with the reason dropped on the floor.
fn blocked(e: String) -> (bool, bool) {
    if let Ok(mut v) = blocked_reasons().lock() {
        v.push(e.trim().chars().take(300).collect());
    }
    (false, false)
}

impl Probe {
    fn ok(&self) -> bool {
        self.red_fires && self.green_passes
    }
}

/// The declared mutations, keyed by probe name. Kept beside the probes rather than on `Probe`
/// so that adding one does not touch every probe literal in a file six lanes edit at once.
///
/// A key that matches no probe is a HARD FAILURE, never skipped: that is the case where a probe
/// was renamed and its evidence quietly stopped applying to anything.
///
/// air-682 says to start with the hand-over gate, since that is Air's one refusal. Each anchor
/// below names ONE branch of `handover_verdict`, so a mutation cannot pass by taking out the
/// whole guard.
const MUTATIONS: &[(&str, Mutation)] = &[
    // The ledger lane's probes, 2026-08-29. Each anchor was run by hand when the probe was
    // written, and each names ONE branch: the change-only gate, the enumeration, the
    // referenced-day protection, the join's file-and-order keys, the freshness window, the
    // bookkeeping overlap, the push deny.
    (
        "status: an unchanged condition set writes one event line an hour, not one a tick",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: "    if attention_only\n        && !ledger",
            to: "    if false\n        && !ledger",
            also_red: &[],
        },
    ),
    (
        "doctor: every table the ledger has is counted, including one added after this probe was written",
        Mutation {
            // Back to the seven names the list held, which is the old behaviour expressed in
            // the new code path, so the mutant reaches exactly what the probe exercises.
            file: "crates/cli/src/cmd/doctor.rs",
            from: "AND name NOT LIKE 'sqlite_%' ORDER BY name",
            to: "AND name = 'verify_runs' ORDER BY name",
            also_red: &[],
        },
    ),
    (
        "gc: an old day the ledger points at is kept, an unreadable clock keeps everything, only an unreferenced old day is collected",
        Mutation {
            file: "crates/cli/src/cmd/gc.rs",
            from: "} else if referenced.contains(day) {",
            to: "} else if false && referenced.contains(day) {",
            also_red: &[],
        },
    ),
    (
        "audit: a warned session that keeps editing the file reads IGNORED; one that stops reads heeded",
        Mutation {
            // Keep the session key, drop the file and the ordering: the join stops being a join
            // without the guard disappearing, which is the wrong-path trap this avoids.
            file: "crates/cli/src/cmd/audit.rs",
            from: ".filter(|(a, s, p)| s == sid && p == path && a > at)",
            to: ".filter(|(_a, s, _p)| s == sid)",
            also_red: &[],
        },
    ),
    (
        "status: a poll tick with fresh cached counts calls bd not at all; a stale or empty cache pays once",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: ".is_some_and(|age| age < max_age_min)",
            to: ".is_some_and(|age| age < 0)",
            also_red: &[],
        },
    ),
    (
        "audit: every registered trace is claimed by exactly one mechanism and none is also bookkeeping",
        Mutation {
            // Restore the overlap this probe found for real: `reported` was bookkeeping AND the
            // audit mechanism's trace, which is why `air gc` fired invisibly (air-8br).
            file: "crates/cli/src/cmd/audit.rs",
            from: "    \"released\",\n    \"stopped\",",
            to: "    \"released\",\n    \"reported\",\n    \"stopped\",",
            also_red: &[],
        },
    ),
    (
        "launch: neither role may push; neither is denied `git commit` (the boundary is the remote, not main)",
        Mutation {
            // Put the commit deny back: the probe's GREEN half is what falls, which is the half
            // the owner's ruling changed (air-iy1).
            file: "crates/cli/src/cmd/launch.rs",
            from: "pub const COORDINATOR_DENY: &[&str] = &[\"Bash(git push *)\"];",
            to: "pub const COORDINATOR_DENY: &[&str] = &[\"Bash(git push *)\", \"Bash(git commit *)\"];",
            also_red: &[],
        },
    ),
    (
        "doctor: an install record older than the binary is named with both versions, the unread notices and the fix; a current or absent one is not",
        Mutation {
            // Invert the surface comparison: a record behind on surface version alone stops
            // lagging, which is the probe's first red record. The crate comparison is untouched,
            // so the anchor names ONE branch and the probe's second red record still fires.
            file: "crates/cli/src/cmd/install.rs",
            from: "    let surface_older = rec.surface_version.is_some_and(|s| s < surface_version);",
            to: "    let surface_older = rec.surface_version.is_some_and(|s| s > surface_version);",
            also_red: &[],
        },
    ),
    (
        "land: the acceptance read's bd budget grows with the id count, and the refusal names the count, the budget and AIR_BD_TIMEOUT_MS",
        Mutation {
            // Drop the per-id term: the budget is the base again, whatever the count, which
            // is the flat 10 s that refused the adopter's batch. The probe's scaled read then
            // times out exactly as its flat one does.
            file: "crates/cli/src/cmd/status.rs",
            from: "    base.saturating_add(per_id.saturating_mul(n))",
            to: "    base.saturating_add(per_id.saturating_mul(n.min(0)))",
            also_red: &[],
        },
    ),
    (
        "hook: the gate reads digest_dir from the worktree root, so a close from a subdirectory says what the root says",
        Mutation {
            // Keep the cwd as the tool gave it: the root is looked up and thrown away, which
            // is the code before air-1r6. The probe's subdirectory run then differs from the
            // root run.
            file: "crates/cli/src/cmd/hook.rs",
            from: "    let cwd = git::toplevel(&cwd).unwrap_or(cwd);",
            to: "    let cwd = git::toplevel(&cwd).map(|_| cwd.clone()).unwrap_or(cwd);",
            also_red: &[],
        },
    ),
    (
        "hook: no session ever reads stuck; a permission request changes no state and nothing is named for it",
        Mutation {
            // Bring the deleted arm back: a PermissionRequest writes `stuck` again. The probe's
            // real hook run sees the state change; nothing else in the suite drives that event.
            file: "crates/cli/src/cmd/hook.rs",
            from: "        _ => Dispatched::new(\n            HookOutcome::Allow { context: None },\n            \"ignored\",\n            \"no handler\",\n        ),",
            to: "        HookEvent::PermissionRequest => {\n            let prev = set_session(ledger, input, worker, \"stuck\", None)?;\n            Dispatched::new(HookOutcome::Allow { context: None }, \"stuck\", transition(&prev, \"stuck\"))\n        }\n        _ => Dispatched::new(\n            HookOutcome::Allow { context: None },\n            \"ignored\",\n            \"no handler\",\n        ),",
            also_red: &[],
        },
    ),
    (
        "record: a red at a batch head is reported by member and lands nothing; a red at a worker's own head is not a batch",
        Mutation {
            // Drop the members filter: every red run becomes a "batch", including a worker's
            // red at its own head. The probe's green half is what falls.
            file: "crates/cli/src/cmd/batch.rs",
            from: "        .filter(|r| !r.members.is_empty())",
            to: "        .filter(|r| r.members.is_empty() || !r.members.is_empty())",
            also_red: &[],
        },
    ),
    (
        "gate: a batch green that contains main and every commit of the bead closes it; one cut before the last commit is refused naming that commit",
        Mutation {
            // "every commit" becomes "any commit": a batch cut before the worker's last commit
            // would close the bead. The per-bead rule the owner's note is about.
            file: "crates/cli/src/cmd/batch.rs",
            from: "        let covers_all = c.contains.len() == commits.len() && c.contains.iter().all(|x| *x);",
            to: "        let covers_all = c.contains.len() == commits.len() && c.contains.iter().any(|x| *x);",
            also_red: &[],
        },
    ),
    (
        "launch: a worker is denied AskUserQuestion and the hook counts the attempt; the coordinator is not, and air capture stays open",
        Mutation {
            // Drop the entry: the deny is gone and the matcher alone remains, which is the
            // count without the refusal.
            file: "crates/cli/src/cmd/launch.rs",
            from: "    \"AskUserQuestion\",\n];",
            to: "];",
            also_red: &[],
        },
    ),
    (
        "gate: verify-green-at-head",
        Mutation {
            // Re-anchored 2026-09-06 (air-g7e): air-80x.1 rewrote this branch to
            // `let green = f.green_at_head || f.batch_green.is_some();`, so the old anchor
            // matched nothing and this mutation had been BROKEN, silently, ever since. The
            // probe kept passing and kept having no evidence behind it.
            file: "crates/hooks/src/gate.rs",
            from: "    if !green {",
            to: "    if false {",
            // The enforced-gate and close-with-proof probes drive the same refusal end to end,
            // and so does the env-delivery probe, which runs the real hook (air-9dg).
            also_red: &[
                // air-g7e: air-80x.1 made this branch SHARED — `let green = f.green_at_head
                // || f.batch_green.is_some()` — so neutralising it takes the batch gate with
                // it, correctly. Declared rather than worked around: the mutation really is
                // wider than one probe now, and saying so is the honest form of that.
                "gate: a batch green that contains main and every commit of the bead closes it; one cut before the last commit is refused naming that commit",
                "gate: AIR_ENFORCE=1 denies bd update -s awaiting_review without green at HEAD (names the fix); allows with green",
                "gate: two closes on one unchanged HEAD cost one verify; a commit demands a new one and clears",
                "launch: Air's env survives a pass-through --settings and reaches the hook, which refuses a close without green",
                // Its "with neither digest nor green it is still refused" half is this rule
                // (air-60x; declared by air-8d7).
                "handover: a superseding branch hands over by its `Bead:` trailer; with neither digest nor green it is still refused, and never told to claim",
            ],
        },
    ),
    (
        "gate: main-merged",
        Mutation {
            file: "crates/hooks/src/gate.rs",
            from: "if !f.main_is_ancestor {",
            to: "if false {",
            also_red: &[
                "gate: a refusal after a landing names the landing that moved main, when and from whom; the fix is unchanged",
                // Both assert on the refusal this rule produces (air-75u, air-5wq; declared
                // by air-8d7).
                "hook: a session is who its launcher says, not where its shell sits; a refusal names whose tree it is about",
                "handover: the ok line names the main it checked against, and a refusal after main moves names the new one",
            ],
        },
    ),
    (
        "gate: a refusal after a landing names the landing that moved main, when and from whom; the fix is unchanged",
        Mutation {
            // Never name the landing, which is the pre-fix wording exactly (air-4up): the
            // refusal still fires and still names the fix, so `gate: main-merged` stays
            // GREEN under it — which shows this reaches the wording and not the check.
            file: "crates/hooks/src/gate.rs",
            from: "if let Some(m) = f.main_moved.as_ref() {",
            to: "if let Some(m) = f.main_moved.as_ref().filter(|_| false) {",
            also_red: &[],
        },
    ),
    (
        "hook: a session is who its launcher says, not where its shell sits; a refusal names whose tree it is about",
        Mutation {
            // Ignore the launcher's role, which is the pre-fix rule exactly (air-75u): the
            // identity falls through to the checkout the shell is in. One guard, it compiles,
            // and the worker arm and the fallback are untouched.
            file: "crates/cli/src/cmd/hook.rs",
            from: "        (Some(\"coordinator\"), _) => \"main\".to_string(),",
            to: "        (Some(\"coordinator\"), _) if false => \"main\".to_string(),",
            also_red: &[],
        },
    ),
    (
        "hook: every `air <subcommand>` a shipped hook or status string names is a real subcommand",
        Mutation {
            // The pre-fix advice, verbatim (air-w91): the string ships a command that does not
            // exist. The probe reads hook.rs at build time, so the mutated source is what it
            // sees; nothing else reads that string, so every other probe stays GREEN.
            file: "crates/cli/src/cmd/hook.rs",
            from: "(`air holdings` says who is in the file; `air status` shows their head and whether it is green)",
            to: "(run `air peer <name>` for their green sha)",
            also_red: &[],
        },
    ),
    (
        "status: the unit tests hold one instant and derive every age from Thresholds::default()",
        Mutation {
            // Put one of the copied literals back beside NOW, which is the pre-fix shape
            // exactly (air-an9). Only this probe reads status.rs's test module as text, so
            // every other probe stays GREEN.
            file: "crates/cli/src/cmd/status.rs",
            from: "    const NOW: &str = \"2026-08-20T12:00:00Z\";\n",
            to: "    const NOW: &str = \"2026-08-20T12:00:00Z\";\n    const T_30: &str = \"2026-08-20T11:30:00Z\";\n",
            also_red: &[],
        },
    ),
    (
        "handover: the ok line names the main it checked against, and a refusal after main moves names the new one",
        Mutation {
            // The ok line without the main it was true of: the pre-fix line exactly
            // (air-5wq). One argument; the refusal side is untouched, so the green half
            // and `gate: main-merged` stay GREEN, which shows this reaches the ok line alone.
            file: "crates/hooks/src/gate.rs",
            from: "containing_main(&f.main_sha)",
            to: "containing_main(\"\")",
            also_red: &[],
        },
    ),
    (
        "land: a refused landing publishes no landed beads and silences no refutation; a landed one publishes all of them",
        Mutation {
            // The old denylist of one, exactly as it stood: only `in-flight` is skipped, so a
            // refused row is read as a landing again. One branch, in the reader that was wrong.
            file: "crates/ledger/src/landings.rs",
            from: "            if !l.landed() {",
            to: "            if l.result == \"in-flight\" {",
            also_red: &[],
        },
    ),
    (
        "verify: a run in flight is named by status and refused by land with its pid and the override; nothing running is silent and a dead pid clears",
        Mutation {
            // Back to never refusing: the one branch that turns a live in-flight run into a
            // refusal (air-1bm). The status line and the prune are untouched, so only the
            // refusal half of the red case falls.
            file: "crates/cli/src/cmd/land.rs",
            from: "    if flights.is_empty() {\n        return None;\n    }",
            to: "    if true {\n        return None;\n    }",
            also_red: &[],
        },
    ),
    (
        "land: naming one bead lands and records every bead its branch carries, once each; an unnamed bead is still refused and another branch is not swept in",
        Mutation {
            // Back to filtering the branch's landings by the bead typed: the exact line
            // The adopter hit, in the one place the selection is expanded.
            file: "crates/cli/src/cmd/land.rs",
            from: "        .filter(|l| chosen.contains(l.worker.as_str()))",
            to: "        .filter(|l| chosen.contains(l.worker.as_str()) && beads.contains(&l.bead))",
            // The air-09b probe's green half asserts the same expansion for a single named
            // bead (it used to assert the defect, `v.len() == 1`), so it falls with this too.
            also_red: &[
                "land: a bead on two branches is refused naming each with --worker; --worker lands that branch with every bead it carries; a bead on one blocked branch is still refused with its fix",
            ],
        },
    ),
    (
        "release: reopening a bead clears its assignee in the same bd process, so anyone can claim it",
        Mutation {
            // Back to reopening alone: the exact write that left and air-an9
            // unclaimable.
            file: "crates/bd/src/lib.rs",
            from: "    [\"update\", id, \"-s\", \"open\", \"-a\", \"\"]",
            to: "    [\"update\", id, \"-s\", \"open\"]",
            also_red: &[],
        },
    ),
    (
        "claim: a bd timeout is retried once and a refusal never is; two timeouts stop at two attempts",
        Mutation {
            // No retry at all: the timeout is returned as it came. The refusal half and the
            // two-attempt cap are untouched, so only the red case falls.
            file: "crates/cli/src/cmd/claim.rs",
            from: "        Err(BdError::Timeout(_)) => {\n            on_retry();\n            (f(), true)\n        }",
            to: "        Err(BdError::Timeout(t)) => (Err(BdError::Timeout(t)), false),",
            also_red: &[],
        },
    ),
    (
        "land: bd not answering about acceptance refuses before the merge; a bead that states none still lands as 'none'",
        Mutation {
            // The old arm: a bd error becomes one empty clause list per bead, which the
            // judge reads as "states no acceptance criteria". Exactly the row the adopter got.
            file: "crates/cli/src/cmd/land.rs",
            from: "        Ok(c) => Ok(c),\n        Err(e) => Err(format!(",
            to: "        Ok(c) => Ok(c),\n        Err(_) => Ok(vec![Vec::new(); beads.len()]),\n        #[allow(unreachable_patterns)]\n        Err(e) => Err(format!(",
            also_red: &[],
        },
    ),
    (
        "record: a run killed by signal (143/137) records no verdict at its sha; an exit-2 failure is still red",
        Mutation {
            // Let killed rows back into every green/red/flaky query: the one clause that
            // makes a kill no verdict, in the one place it is spelled.
            file: "crates/ledger/src/verify.rs",
            from: "const NOT_KILLED: &str = \"exit_code NOT IN (137, 143)\";",
            to: "const NOT_KILLED: &str = \"1=1\";",
            also_red: &[],
        },
    ),
    (
        "green: a landing reads green from its tree only where the repo declares verify_key tree; an unverified tree never does",
        Mutation {
            // Let a tree green count under the default key. That is the silent upgrade
            // air-7wf refused to ship: the adopter's citation gate would have started passing
            // beads it never checked. One arm, and the one the probe's red half is about.
            file: "crates/cli/src/cmd/green.rs",
            from: "Some(GreenAt::Tree(_)) => self.key == Key::Tree,",
            to: "Some(GreenAt::Tree(_)) => true,",
            also_red: &[],
        },
    ),
    (
        "launch: a task is the prompt; no task means no prompt, so an untriggered worker never runs",
        Mutation {
            // Invert the blank-task test: a real task stops becoming the prompt, and a blank one
            // starts. One branch, and the one this probe is about (air-7q5).
            file: "crates/cli/src/cmd/launch.rs",
            from: "if let Some(t) = prompt.filter(|t| !t.trim().is_empty()) {",
            to: "if let Some(t) = prompt.filter(|t| t.trim().is_empty()) {",
            also_red: &[
                "launch: --task reaches claude as the prompt by file; the task text is not in argv, and claude runs in the worktree Air made rather than being handed it",
            ],
        },
    ),
    (
        "launch: --task reaches claude as the prompt by file; the task text is not in argv, and claude runs in the worktree Air made rather than being handed it",
        Mutation {
            // Put the task back into argv, which is the shape that killed the adopter's workers
            // (air-er0). The file is still written; only the prompt regresses.
            file: "crates/cli/src/cmd/launch.rs",
            from: "Ok(p) => Some(task_prompt(&p)),",
            to: "Ok(_p) => Some(t.to_string()),",
            also_red: &[],
        },
    ),
    (
        "launch: Air's env survives a pass-through --settings and reaches the hook, which refuses a close without green",
        Mutation {
            // The launcher stops delivering enforcement: the stub's recorded environment
            // carries AIR_ENFORCE=0, and the real hook run in it advises instead of refusing
            // (air-9dg). The merged --settings blob regresses with it, so the probe cannot
            // pass on the blob alone.
            file: "crates/cli/src/cmd/launch.rs",
            from: "(\"AIR_ENFORCE\", \"1\"),",
            to: "(\"AIR_ENFORCE\", \"0\"),",
            also_red: &[],
        },
    ),
    (
        "gate: the digest refusal says the digest commit moves HEAD off the green and names the order, only when a green is at HEAD",
        Mutation {
            // The note stops being conditional on the green: it goes silent for the worker
            // who has one (air-yol's report) and would speak for the one who has not.
            file: "crates/hooks/src/gate.rs",
            from: "let order_note = if f.green_at_head {",
            to: "let order_note = if !f.green_at_head {",
            also_red: &[],
        },
    ),
    (
        "worktree: Air's worktree carries .worktreeinclude's files and builds; git alone does not; removal refuses uncommitted work and keeps the branch",
        Mutation {
            // Air stops copying: the worktree is the naive one and its build fails, which is
            // The adopter's fleet that does not compile (air-fdz).
            file: "crates/cli/src/cmd/worktree.rs",
            from: "let copied = copy_included(main, &path)?;",
            to: "let copied = Copied::default();",
            also_red: &[],
        },
    ),
    (
        "events: bd_calls on a line is that event's own count, not the process's running total",
        Mutation {
            // `take` hands back the lifetime total again, which is what every `air mcp` line
            // carried before air-bp0.
            file: "crates/bd/src/lib.rs",
            from: "calls.saturating_sub(prev_calls))",
            to: "calls.saturating_sub(prev_calls.min(0)))",
            also_red: &[],
        },
    ),
    (
        "gate: the named bead must be claimed by the worker or carried by a `Bead:` trailer in main..HEAD",
        Mutation {
            file: "crates/hooks/src/gate.rs",
            from: "if !f.bead_claimed_or_carried {",
            to: "if false {",
            // The superseding-branch probe's "never told to claim" half reads this refusal
            // (air-60x; declared by air-8d7).
            also_red: &[
                "handover: a superseding branch hands over by its `Bead:` trailer; with neither digest nor green it is still refused, and never told to claim",
            ],
        },
    ),
    (
        "gate: a digest counts when it declares its bead; a different bead, a touch, or a name match do not",
        Mutation {
            // Not `fn declared_bead(` -> a rename: that does not build, and a mutation that
            // does not build is failure mode 1, full marks and no information. This neutralises
            // the ONE rule the probe's red case names — that a digest declaring a different
            // bead is not this bead's digest — and leaves the cutoff and mtime paths alone.
            file: "crates/cli/src/cmd/handover.rs",
            from: "Some(b) => beads.contains(&b),",
            to: "Some(_) => true,",
            also_red: &[],
        },
    ),
    (
        "attention: idle-without-claim fires for a live session, not a dead one",
        Mutation {
            // The one clause air-d10 added. Not the whole arm: taking that out would silence
            // the condition entirely and prove only that the probe is connected.
            file: "crates/cli/src/cmd/status.rs",
            from: "sess.pid_alive != Some(false)",
            to: "true",
            also_red: &[],
        },
    ),
    (
        "doctor: a dated rule says so when its cutoff has passed",
        Mutation {
            // The comparison itself, so a rule's date stops being read. Renaming the function
            // would not build, which proves nothing (failure mode 1).
            file: "crates/cli/src/cmd/doctor.rs",
            from: "expired: date.parse::<jiff::Timestamp>().is_ok_and(|t| now >= t),",
            to: "expired: false,",
            also_red: &[],
        },
    ),
    (
        "claim: a closed bead stops alarming; awaiting_review still holds it",
        Mutation {
            // Widen `closes_bead` back to every hand-over, so `-s awaiting_review` releases
            // the claim too. That is the air-3eu regression the probe's green half is about,
            // and it leaves the close path working, which is what makes it one branch.
            file: "crates/cli/src/cmd/hook.rs",
            from: "closing.then(|| handover_bead(cmd)).flatten()",
            to: "handover_bead(cmd)",
            also_red: &[],
        },
    ),
    (
        "attention: three stuck claims on one worker are one line, not three",
        Mutation {
            // Collapse by throwing claims away instead of by naming them: one line, but it
            // reports one bead of three. The probe's red half asks for all three by name.
            file: "crates/cli/src/cmd/status.rs",
            from: ".filter(|c| c.handover_attempts > 0)\n                .collect()",
            to: ".filter(|c| c.handover_attempts > 0)\n                .take(1)\n                .collect()",
            also_red: &[],
        },
    ),
    (
        "status: the bd budget is derived from bd's measured cost, not a constant",
        Mutation {
            // Stop reading the measurement, so every budget is the floor. The function still
            // returns a Duration and the cap still holds; only the derivation goes.
            file: "crates/cli/src/cmd/bd_latency.rs",
            from: ".and_then(|m| m.checked_mul(4))",
            to: ".and_then(|_| None)",
            also_red: &[],
        },
    ),
    (
        "install: a binary below the repo's surface version is refused; equal, higher and unrecorded write",
        Mutation {
            // Allow every write: the pre-air-w9d world, where only the forward direction was
            // computed and an older binary wrote over a newer record reporting success. The
            // green half legitimately survives (it asserts writes ARE allowed), so only the
            // red half falls, which is what naming one branch means.
            file: "crates/cli/src/cmd/install.rs",
            from: "theirs.is_none_or(|t| mine >= t)",
            to: "theirs.is_none_or(|_| true)",
            also_red: &[],
        },
    ),
    (
        "install: a repo at yesterday's surface is told what changed; a current one is told nothing",
        Mutation {
            // A repo that has recorded ANYTHING is told nothing, while a repo recorded at
            // nothing still sees every change (air-8d7). The earlier form, `false`, took the
            // whole diff out and so also felled `install: an older recorded surface diffs`,
            // whose red half is `surface_diff(&[])`; that is the shared rule, and the report
            // read VACUOUS. This one reaches the branch only this probe drives: a partially
            // told repo. The green half still passes (a fully told repo is told nothing).
            file: "crates/cli/src/cmd/install.rs",
            from: "!known.iter().any(|k| k == c.id)",
            to: "known.is_empty()",
            also_red: &[],
        },
    ),
    (
        "lease: a defect reaches the waiter, never the holder, and nobody waiting is silent",
        Mutation {
            // Address the condition to the holder again — the whole defect air-q9c fixed.
            // The condition still fires and still says the same thing; only the name on it
            // changes, so this cannot pass by silencing the arm.
            file: "crates/cli/src/cmd/status.rs",
            from: "worker: who.clone(),",
            to: "worker: l.worker.clone(),",
            also_red: &[],
        },
    ),
    (
        "traffic: SendMessage reaches the hook and the audit sums it per worker",
        Mutation {
            // The matcher, which is the thing that made the count zero in the first place.
            file: "crates/cli/src/cmd/install.rs",
            // Re-anchored 2026-09-06 (air-g7e): air-bm3 added `AskUserQuestion` to the
            // matcher, so this anchor stopped matching and the mutation had been BROKEN
            // since. Anchored on the `SendMessage` token alone now, which is the thing the
            // rule is about, so the next tool added to the matcher does not break it again.
            from: "|SendMessage|AskUserQuestion\")",
            to: "|AskUserQuestion\")",
            also_red: &[],
        },
    ),
    (
        "verify: a run in flight is named by status and refused by land with its pid and the override; nothing running is silent and a dead pid clears",
        Mutation {
            // The rule: an in-flight row whose process is gone is not a run in flight. Keep
            // the shape and neutralise only the liveness test, so the mutation cannot pass by
            // taking out the whole prune (air-4cr).
            file: "crates/ledger/src/verify.rs",
            from: "if f.pid.is_none_or(&alive) {",
            to: "if f.pid.is_none_or(|_| true) {",
            also_red: &[],
        },
    ),
    (
        "land: a landing says in-flight from the merge until it reports, a killed one says so with the rewind sha, and reporting retires the row",
        Mutation {
            // The rule: `in-flight` is the state that means "merged, outcome not yet
            // recorded". One branch, and not the near-identical test in `landed_open` two
            // functions below, which is a different rule about which landing decides a bead
            // (air-bxe).
            file: "crates/ledger/src/landings.rs",
            from: ".filter(|l| l.result == \"in-flight\")",
            to: ".filter(|l| l.result == \"landed\")",
            also_red: &[],
        },
    ),
    (
        "land: the role is where the process is, so --repo at the main checkout does not make a worker the coordinator",
        Mutation {
            // Exactly the pre-fix behaviour: decide on what `--repo` resolved to instead of on
            // where the process runs. This is the defect air-29a found, so the probe is
            // evidence only if it falls to it. `land: worker, dirty main, …` legitimately
            // stays GREEN — its two cases have `where_i_am` and `where_i_pointed` agreeing, so
            // this mutation does not reach them.
            file: "crates/cli/src/cmd/land.rs",
            from: "if super::hook::role_for(here) == \"coordinator\" {",
            to: "if super::hook::role_for(c.where_i_pointed) == \"coordinator\" {",
            also_red: &[],
        },
    ),
    (
        "attention: idle-without-claim counts beads the worker may claim, not bd's raw ready set",
        Mutation {
            // Put the condition back on bd's raw count. Exactly the pre-fix behaviour, one
            // branch, and it compiles. The two threshold probes give the counts the same value
            // deliberately, so they stay GREEN under it — which is what shows this mutation
            // reaches the counting rule and not the threshold beside it.
            file: "crates/cli/src/cmd/status.rs",
            from: "&& s.claimable_depth.is_some_and(|n| n > 0)",
            to: "&& s.ready_depth.is_some_and(|n| n > 0)",
            also_red: &[],
        },
    ),
    (
        "land: Air runs exactly one git merge and it is --ff-only, so no Air command can see a conflict",
        Mutation {
            // Reintroduce the three-way merge air-odv removed. It compiles, it is exactly the
            // regression the claim guards against, and it is one line rather than a blanket
            // flag. `--no-ff` against a divergent branch is precisely what CAN conflict.
            file: "crates/cli/src/cmd/land.rs",
            from: "git::run(repo, &[\"merge\", \"--ff-only\", &merge])",
            to: "git::run(repo, &[\"merge\", \"--no-ff\", &merge])",
            also_red: &[],
        },
    ),
    (
        "land: main moves only for a branch that contains main AND is green at its head, which is why no verify runs there",
        Mutation {
            // Drop the containment half of the conjunction. It compiles, it reaches exactly the
            // branch under test, and it is the state that would let main move to a commit whose
            // tree nothing has verified — which is what the removed rewind used to cover for.
            // `land: worker, dirty main, …` shares this rule and is expected to fall with it.
            file: "crates/cli/src/cmd/land.rs",
            from: "    if !f.contains_main {",
            to: "    if false {",
            also_red: &[
                "land: worker, dirty main, stale branch and a green off the head are all refused with a fix; a clean green passes",
            ],
        },
    ),
    (
        "landable: a branch green with main merged pushes once per head, not while it sits",
        Mutation {
            // The rule: what makes a branch landable is its HEAD, so nothing else may enter
            // the fingerprint. Putting the bead count back in is the exact regression — the
            // branch re-pushes when a bead joins, which is not a change in whether it can land
            // (air-03w, air-s7c).
            file: "crates/cli/src/cmd/status.rs",
            from: "fingerprint: format!(\"{worker}@{head}\"),",
            to: "fingerprint: format!(\"{worker}@{head}/{}\", beads.len()),",
            also_red: &[],
        },
    ),
    (
        "status: landed-not-closed names only the clauses the merge contradicts; a bead Air merely could not read makes no CONTRADICTS claim",
        Mutation {
            // Render the row's whole `why` again, which is exactly the pre-fix line (air-ppf):
            // every unreadable clause back under the CONTRADICTS headline. One binding, it
            // compiles, and `landed_open` still filters on `refuted`, so the only-unreadable
            // half stays silent under it — which shows the mutation reaches the rendering rule
            // and not the ledger's filter beside it.
            file: "crates/cli/src/cmd/status.rs",
            from: "let (bead, why) = (&o.bead, &o.contradicted);",
            to: "let (bead, why) = (&o.bead, &o.why);",
            also_red: &[],
        },
    ),
    (
        "acceptance: a path-like token that is no file at the landed commit is unreadable, not refuted; an existing untouched file still is",
        Mutation {
            // Every missing path counts as an untouched file again, which is exactly the
            // pre-fix rule (air-dqa): the tree is never consulted. One closure, it compiles,
            // and the two landed-not-closed probes name files that ARE in their tree, so they
            // stay GREEN under it — which shows this reaches the resolution rule alone.
            file: "crates/cli/src/cmd/acceptance.rs",
            from: ".partition(|p| ev.tree.iter().any(|t| t == *p));",
            to: ".partition(|_p| true);",
            also_red: &[],
        },
    ),
    (
        "land: a bead on two branches is refused naming each with --worker; --worker lands that branch with every bead it carries; a bead on one blocked branch is still refused with its fix",
        Mutation {
            // Never see a bead as ambiguous, which is the pre-fix rule exactly (air-09b): two
            // landable carriers both go to the batch and the oldest lands first; a blocked
            // carrier beside a landable one is refused as blocked. One comparison, it
            // compiles, and the `--worker` path and the single-carrier paths are untouched by
            // it, which is what shows it reaches the ambiguity rule alone.
            file: "crates/cli/src/cmd/land.rs",
            from: "        if carriers.len() > 1 {",
            to: "        if carriers.len() > 99 {",
            also_red: &[],
        },
    ),
    // The budget lane's probes, 2026-09-06 (air-d75). Each anchor names ONE thing: the drain,
    // one catalogue row, the hook's own recording, the derived pairable set.
    (
        "budgets: one sample per wait against its own budget, and a take drains",
        Mutation {
            // Stop draining and hand back a copy, which is air-bp0 exactly: every later event
            // line in the process restamps the same waits. Compiles — the guard derefs to the
            // map — and the probe's second `take` then still holds the first one's waits.
            file: "crates/ledger/src/budgets.rs",
            from: "    std::mem::take(&mut t)",
            to: "    t.clone()",
            also_red: &[],
        },
    ),
    (
        "budgets: every recordable budget has a catalogue row, and the row says which way it fails",
        Mutation {
            // Rename one row so a recordable name has no catalogue row: the budget is still
            // measured and `air audit` reports it as uncatalogued instead of naming its fail
            // direction. One row, and the OTHER nine are untouched.
            file: "crates/cli/src/cmd/budgets.rs",
            from: "        name: air_ledger::budgets::SQLITE_LOCK,",
            to: "        name: \"sqlite-lock-renamed\",",
            also_red: &[],
        },
    ),
    (
        "budgets: a real hook invocation records its own wall clock and the git calls it made",
        Mutation {
            // Record the hook's wall clock under a name nothing reads, so the invocation is
            // still timed and the event line no longer carries `budgets.hook` — which is the
            // state before this bead. The `git` half is untouched, so the probe's GREEN half
            // stays green and the mutation is shown to reach the hook's own row alone.
            file: "crates/cli/src/cmd/hook.rs",
            from: "        air_ledger::budgets::HOOK,",
            to: "        \"hook-unrecorded\",",
            also_red: &[],
        },
    ),
    (
        "budgets: an unpaired hook is counted from the installed matchers, in both directions",
        Mutation {
            // Pair every tool the PreToolUse matcher names, whether PostToolUse covers it or
            // not — the hardcoded-list mistake, written as code. `SendMessage` then has a Pre
            // that nothing can ever answer and reads as a lost hook.
            file: "crates/cli/src/cmd/budgets.rs",
            from: "        .filter(|t| post.contains(t))",
            to: "        .filter(|t| post.contains(t) || !t.is_empty())",
            also_red: &[],
        },
    ),
    (
        "release: a lane's notice passes verify and waits for the round; the release check refuses it, naming the row",
        Mutation {
            // Put the equality back: a notice beyond the last row fails `make verify` again,
            // which is the state that cost nineteen releases and five row collisions in a day.
            // The release check is untouched, so the probe's GREEN half stays green and the
            // mutation is shown to reach the verify-time rule alone.
            file: "crates/cli/src/cmd/install.rs",
            from: "    len >= last",
            to: "    len == last",
            also_red: &[],
        },
    ),
    (
        "hook: a worker's edit outside its worktree is denied naming the path; inside is allowed and the coordinator in main is never fenced",
        Mutation {
            // Fence the coordinator instead of the worker: one comparison, it compiles, and
            // the fence, the path arithmetic and the message are all untouched. The worker
            // stops being fenced (the probe's RED half falls) and the coordinator still is
            // not, because its checkout IS the root it would be measured against — so the
            // GREEN half survives, which is what shows the anchor reaches the role gate alone
            // rather than taking out the check.
            file: "crates/cli/src/cmd/hook.rs",
            from: "    if let Some(abs) = input.edited_path()\n        && role_for(worker) == \"worker\"",
            to: "    if let Some(abs) = input.edited_path()\n        && role_for(worker) == \"coordinator\"",
            also_red: &[],
        },
    ),
    (
        "launch: metis is attached to the coordinator and to no worker; a plugin dir that is not a directory is dropped, not passed",
        Mutation {
            // Pass a declared plugin dir whether or not it exists, which is the shape that
            // loads nothing and says nothing. The MCP half and the worker half are untouched,
            // so the probe's RED half (a real directory IS passed) survives and only the
            // dropped-path half falls.
            file: "crates/cli/src/cmd/metis.rs",
            from: "        Some(d) if Path::new(d).is_dir() => (Some(d.to_string()), None),",
            to: "        Some(d) if !d.is_empty() => (Some(d.to_string()), None),",
            also_red: &[],
        },
    ),
    (
        "status: an initiative is a declared line, not a mention, and the count that reads it is not a gate",
        Mutation {
            // Read the initiative out of prose instead of off a declared line: any description
            // containing the word counts as declaring one. It compiles, the count still runs,
            // and a bead that merely mentions an initiative stops being counted — the
            // permitting direction, and the one the anti-brittleness skill names.
            file: "crates/cli/src/cmd/metis.rs",
            from: "        let rest = l.trim().strip_prefix(\"initiative:\")?;",
            to: "        let rest = l.trim().split_once(\"initiative\").map(|(_, r)| r)?;",
            also_red: &[],
        },
    ),
    (
        "selftest: every declared mutation still anchors exactly once in the file it names",
        Mutation {
            // Accept a dead anchor as fine, which is the state the registry was actually in
            // for days: two mutations anchored nothing and the suite printed PASS for both
            // probes anyway. The ambiguous case is untouched, so the mutation reaches the
            // zero-occurrence rule alone.
            file: "crates/cli/src/cmd/selftest.rs",
            from: "match text.matches(m.from).count() {\n                1 => {}",
            to: "match text.matches(m.from).count() {\n                0 | 1 => {}",
            also_red: &[],
        },
    ),
    (
        "privacy: a tracked line naming an adopter is refused with its file and line; a clean tree and a clone with no list are not",
        Mutation {
            // Match case-sensitively, which is the grep everyone writes first and the one that
            // let an upper-case row through the sweep. The declared-name reader and the refusal
            // text are untouched, so the probe's GREEN half survives and only the case half
            // falls.
            file: "crates/cli/src/cmd/privacy.rs",
            from: "            let low = line.to_ascii_lowercase();",
            to: "            let low = line.to_string();",
            also_red: &[],
        },
    ),
];

pub fn run(json: bool) -> i32 {
    let probes = all_probes();
    let all_ok = probes.iter().all(Probe::ok);
    emit(json, &probes, || {
        let mut s = String::new();
        for p in &probes {
            s.push_str(&format!(
                "{} {}: red {} / green {}\n",
                if p.ok() { "PASS" } else { "FAIL" },
                p.name,
                if p.red_fires { "fires" } else { "SILENT" },
                if p.green_passes { "passes" } else { "BLOCKED" },
            ));
        }
        // air-682: the proven count rides on the ordinary run, so a probe added without a
        // declared mutation is visible without anyone remembering to look for it.
        // air-g7e: a probe that could not RUN says why. Without this a flake and a real
        // break are the same two words, and the only available response is to run it again.
        if let Ok(v) = blocked_reasons().lock()
            && !v.is_empty()
        {
            s.push_str(&format!(
                "\n{} probe(s) could not run; each reason is the fixture's own error, in \
                 evaluation order:\n",
                v.len()
            ));
            for r in v.iter() {
                s.push_str(&format!("  {r}\n"));
            }
        }
        s.push_str(&format!(
            "{} probes, {} with a declared mutation (`air selftest --prove`)",
            probes.len(),
            MUTATIONS.len()
        ));
        s
    });
    if all_ok { 0 } else { 1 }
}

/// What running one declared mutation established. `Broken` is deliberately NOT a failure of the
/// probe: a mutation that cannot be applied or cannot be built has demonstrated nothing about the
/// probe either way, and reporting it as evidence is the adopter's failure mode 1.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "outcome")]
enum Proof {
    /// The probe went red with its rule neutralised, and every probe not named in `also_red`
    /// stayed green.
    Proven,
    /// The mutation applied and built, and the probe stayed GREEN. The probe is vacuous, or the
    /// mutation reached the wrong path.
    Vacuous { detail: String },
    /// The mutation could not be applied or did not build. No information about the probe.
    Broken { detail: String },
}

#[derive(Debug, Serialize)]
struct ProofRow {
    probe: &'static str,
    #[serde(flatten)]
    proof: Proof,
}

/// air-682: run every declared mutation and report any probe that stays green.
///
/// Edits tracked files, so it refuses a dirty tree rather than risk restoring the wrong content,
/// and restores after each mutation whatever the outcome — by writing the bytes it read back,
/// never through git (air-8d7). It used `git checkout --`, which takes the index lock, and one
/// restore lost that lock to a `git status` run beside it and was ignored: the bd-budget
/// mutation stayed applied, that probe went red under every later mutation, and fourteen
/// verdicts read VACUOUS for a collateral that was not theirs. A restore that fails now ends
/// the run with the reason, since every verdict after it would be about a tree nobody chose.
pub fn prove(repo: &Path, json: bool) -> i32 {
    let baseline = all_probes();
    let names: Vec<&str> = baseline.iter().map(|p| p.name).collect();
    let mut rows: Vec<ProofRow> = Vec::new();

    // A mutation naming no probe is a hard failure, never a skip: that is exactly the case
    // where a probe was renamed and its evidence silently stopped applying to anything.
    for (probe, m) in MUTATIONS {
        if !names.contains(probe) {
            rows.push(ProofRow {
                probe,
                proof: Proof::Broken {
                    detail:
                        "no probe by this name; it was renamed and its mutation was left behind"
                            .to_string(),
                },
            });
            continue;
        }
        for other in m.also_red {
            if !names.contains(other) {
                rows.push(ProofRow {
                    probe,
                    proof: Proof::Broken {
                        detail: format!("also_red names no probe: {other}"),
                    },
                });
            }
        }
    }
    if rows.iter().any(|r| matches!(r.proof, Proof::Broken { .. })) {
        return report(json, &rows, baseline.len());
    }

    if !git_clean(repo) {
        eprintln!(
            "air: selftest --prove edits tracked files and needs a clean tree; commit or set your work aside first"
        );
        return 2;
    }

    for (probe, m) in MUTATIONS {
        let path = repo.join(m.file);
        let Ok(original) = std::fs::read_to_string(&path) else {
            rows.push(ProofRow {
                probe,
                proof: Proof::Broken {
                    detail: format!("cannot read {}", m.file),
                },
            });
            continue;
        };
        // Exactly once: an anchor that matches twice mutates a branch nobody chose, which is
        // the wrong-path failure with extra steps.
        let hits = original.matches(m.from).count();
        if hits != 1 {
            rows.push(ProofRow {
                probe,
                proof: Proof::Broken {
                    detail: format!(
                        "anchor occurs {hits} times in {}, expected exactly 1",
                        m.file
                    ),
                },
            });
            continue;
        }
        if std::fs::write(&path, original.replacen(m.from, m.to, 1)).is_err() {
            rows.push(ProofRow {
                probe,
                proof: Proof::Broken {
                    detail: format!("cannot write {}", m.file),
                },
            });
            continue;
        }

        let proof = match build_and_run(repo) {
            Err(detail) => Proof::Broken { detail },
            Ok(mutated) => judge(probe, m, &mutated),
        };
        rows.push(ProofRow { probe, proof });
        if let Err(e) = restore(&path, &original) {
            rows.push(ProofRow {
                probe,
                proof: Proof::Broken {
                    detail: format!(
                        "could not restore {} after its mutation ({e}); stopping here, since \
                         every later verdict would be about a tree nobody chose. Restore it by \
                         hand: git checkout -- {}",
                        m.file, m.file
                    ),
                },
            });
            break;
        }
    }
    report(json, &rows, baseline.len())
}

fn report(json: bool, rows: &[ProofRow], total: usize) -> i32 {
    let proven = rows.iter().filter(|r| r.proof == Proof::Proven).count();
    let broken = rows
        .iter()
        .filter(|r| matches!(r.proof, Proof::Broken { .. }))
        .count();
    let vacuous = rows
        .iter()
        .filter(|r| matches!(r.proof, Proof::Vacuous { .. }))
        .count();
    emit(json, &rows, || {
        let mut s = String::new();
        for r in rows {
            match &r.proof {
                Proof::Proven => s.push_str(&format!("PROVEN  {}\n", r.probe)),
                Proof::Vacuous { detail } => {
                    s.push_str(&format!("VACUOUS {}\n        {detail}\n", r.probe));
                }
                Proof::Broken { detail } => {
                    s.push_str(&format!("BROKEN  {}\n        {detail}\n", r.probe));
                }
            }
        }
        s.push_str(&format!(
            "{proven} proven, {vacuous} vacuous, {broken} broken mutation(s); \
             {} of {total} probes declare one",
            MUTATIONS.len()
        ));
        s
    });
    // A broken mutation is not evidence and not a pass: it needs fixing before the suite means
    // anything. A vacuous probe is the finding this command exists to surface.
    if vacuous == 0 && broken == 0 { 0 } else { 1 }
}

/// Did the mutation put the named probe red and leave the rest alone?
fn judge(probe: &str, m: &Mutation, mutated: &[Probe]) -> Proof {
    let Some(target) = mutated.iter().find(|p| p.name == probe) else {
        return Proof::Broken {
            detail: "probe vanished from the mutated build".to_string(),
        };
    };
    if target.ok() {
        return Proof::Vacuous {
            detail: "probe stayed green with its rule neutralised: it is vacuous, or the mutation reached the wrong path".to_string(),
        };
    }
    // Everything not named must have survived. An unexpected casualty means the mutation took
    // out more than the branch under test, which is how a blanket mutant passes for a real one.
    let collateral: Vec<&str> = mutated
        .iter()
        .filter(|p| !p.ok() && p.name != probe && !m.also_red.contains(&p.name))
        .map(|p| p.name)
        .collect();
    if !collateral.is_empty() {
        return Proof::Vacuous {
            detail: format!(
                "probe went red, but so did {} probe(s) not declared in also_red, so the mutation is wider than the rule: {}",
                collateral.len(),
                collateral.join("; ")
            ),
        };
    }
    let survived: Vec<&&str> = m
        .also_red
        .iter()
        .filter(|n| mutated.iter().any(|p| p.name == **n && p.ok()))
        .collect();
    if !survived.is_empty() {
        return Proof::Vacuous {
            detail: format!(
                "also_red named {} probe(s) that stayed green",
                survived.len()
            ),
        };
    }
    Proof::Proven
}

/// Build the mutated tree and run its own `selftest --json`. `Err` is a BROKEN mutation: a
/// mutant that does not compile scores red for nothing.
fn build_and_run(repo: &Path) -> Result<Vec<Probe>, String> {
    let build = Command::new("cargo")
        .args(["build", "-q", "-p", "air"])
        .current_dir(repo)
        .output()
        .map_err(|e| format!("cargo build did not run: {e}"))?;
    if !build.status.success() {
        let err = String::from_utf8_lossy(&build.stderr);
        return Err(format!(
            "mutant does not build, so it is not evidence: {}",
            err.lines()
                .find(|l| l.contains("error"))
                .unwrap_or("")
                .trim()
        ));
    }
    let out = Command::new("cargo")
        .args(["run", "-q", "-p", "air", "--", "selftest", "--json"])
        .current_dir(repo)
        // The anchor probe checks the registry at rest, and right now exactly one anchor is
        // deliberately not where it says it is: the one being applied. Without this it goes
        // red under every mutation and every one of them reads as VACUOUS (air-g7e).
        .env("AIR_SELFTEST_PROVING", "1")
        .output()
        .map_err(|e| format!("mutated selftest did not run: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Vec<ProbeOut>>(&text)
        .map(|v| {
            v.into_iter()
                .map(|p| Probe {
                    name: Box::leak(p.name.into_boxed_str()),
                    red_fires: p.red_fires,
                    green_passes: p.green_passes,
                })
                .collect()
        })
        .map_err(|e| format!("could not read the mutated selftest output: {e}"))
}

#[derive(serde::Deserialize)]
struct ProbeOut {
    name: String,
    red_fires: bool,
    green_passes: bool,
}

fn git_clean(repo: &Path) -> bool {
    Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(repo)
        .output()
        .map(|o| o.stdout.is_empty())
        .unwrap_or(false)
}

/// Put the file back exactly as it was read, and prove it by reading it again.
fn restore(path: &Path, original: &str) -> Result<(), String> {
    std::fs::write(path, original).map_err(|e| e.to_string())?;
    let back = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if back == original {
        Ok(())
    } else {
        Err("read back differs from what was written".to_string())
    }
}

/// air-7q5: starting a session must not start work. The owner drew the line at launch time, and
/// the mechanism that holds it is that a worker launched with no `--task` gets NO PROMPT: the
/// roles prose reaches it through `--append-system-prompt-file`, which is context rather than a
/// turn, so an untriggered session never runs. Red: with a task, the task is the prompt and the
/// session is triggered. Green: with no task, argv opens on a flag and carries no positional at
/// all, so there is nothing for claude to answer.
fn probe_no_task_no_prompt() -> Probe {
    use crate::cmd::launch::{task_is_prompt, worker_argv_prompt};
    let base = vec![
        "--append-system-prompt-file".to_string(),
        "/r/.air/roles.md".to_string(),
        "--disallowed-tools".to_string(),
        "Bash(git push *)".to_string(),
    ];
    let task = "work air-1";
    let with = worker_argv_prompt(base.clone(), Some(task));
    let without = worker_argv_prompt(base.clone(), None);
    // A blank task is not a task: it must not become an empty prompt either.
    let blank = worker_argv_prompt(base.clone(), Some("   "));
    Probe {
        name: "launch: a task is the prompt; no task means no prompt, so an untriggered worker never runs",
        red_fires: with.first().is_some_and(|a| a == task) && task_is_prompt(&with, task),
        green_passes: without == base
            && blank == base
            && without.first().is_some_and(|a| a.starts_with('-')),
    }
}

/// air-uae: two lease stores that disagree deny work while reporting success. The adopter's
/// `make lease-take` wrote Air's ledger and their guard read `<prefix>-leases/`, so `make api` was
/// refused naming the command that had just succeeded. Neither side said where it was looking.
/// Red: `air lease status` names its own store, with the path, on every run. Green: it names the
/// directory it was actually given rather than a fixed string, so a target repo comparing it
/// against its guard's path gets that repo's answer.
fn probe_lease_store_is_named() -> Probe {
    use crate::cmd::lease::store_line;
    let line = store_line(Path::new("/r/.air"));
    Probe {
        name: "lease: `air lease status` names the store it writes, so a second one is visible",
        red_fires: line.contains("lease store:") && line.contains("/r/.air/ledger.db"),
        green_passes: !store_line(Path::new("/other/.air")).contains("/r/.air"),
    }
}

/// air-air: "which model is this worker on" was a question the coordinator had to ASK, and a
/// wrong model that is invisible costs the round while a visible one costs a relaunch.
///
/// The value is READ, never inferred: a session launched with no `--model` inherits whatever the
/// harness gives it, so anything derived from a settings file would be a guess wearing a fact's
/// grammar. It comes out of the session's own transcript, which names the model on every
/// assistant message.
///
/// Red: two sessions on different models are distinguishable, and the launch flag reaches the
/// argv as a flag rather than as one more deny-list value (air-2ct). Green: a transcript that has
/// not named a model yet yields None, so the recorded value is left alone rather than blanked —
/// an honest unknown instead of an empty string.
fn probe_model_is_recorded_per_session() -> Probe {
    use crate::cmd::hook::model_in_transcript;
    let line = |m: &str| format!(r#"{{"type":"assistant","message":{{"model":"{m}","id":"x"}}}}"#);
    let a = model_in_transcript(&line("claude-opus-5"));
    let b = model_in_transcript(&line("claude-haiku-4-5-20251001"));
    // The launch flag must survive as a FLAG: appended bare after the variadic --disallowed-tools
    // it would be read as another deny rule.
    let argv = crate::with_model(Some("claude-opus-5"), &["--tmux".to_string()]);
    let red = a.as_deref() == Some("claude-opus-5")
        && b.as_deref() == Some("claude-haiku-4-5-20251001")
        && a != b
        && argv.first().is_some_and(|x| x == "--model")
        && argv.get(1).is_some_and(|x| x == "claude-opus-5");
    let green = model_in_transcript(r#"{"type":"user","message":{"content":"hi"}}"#).is_none()
        && model_in_transcript("").is_none()
        && model_in_transcript(r#"{"model":""}"#).is_none()
        && crate::with_model(None, &["--tmux".to_string()]) == vec!["--tmux".to_string()];
    Probe {
        name: "status: a session's model is read from its transcript; two models are distinguishable, an unnamed one is not guessed",
        red_fires: red,
        green_passes: green,
    }
}

/// air-sze: every attention kind the code can emit has a registry row, and every registered
/// condition kind is one the code can emit.
///
/// Five kinds shipped with no row. That was worse than an unregistered decision (air-8br): the
/// audit can only count kinds the registry names, so an unregistered condition is not
/// undercounted, it is unseeable — `air audit` reported 14 mechanisms while `attention()` could
/// emit 12 kinds, 5 of which it had never heard of.
///
/// Red: a set comparison both ways, so a new condition without a row fails here rather than
/// going uncounted, and a row for a kind nothing emits fails too. Green: the registry's
/// condition kinds are exactly `kinds::ALL`.
fn probe_every_condition_kind_is_registered() -> Probe {
    use crate::cmd::mechanisms::{Fires, MECHANISMS};
    use crate::cmd::status::kinds;

    let registered: Vec<&str> = MECHANISMS
        .iter()
        .filter_map(|m| match m.fires {
            Fires::Condition(k) => Some(k),
            Fires::Decisions(_) => None,
        })
        .collect();
    let unregistered: Vec<&&str> = kinds::ALL
        .iter()
        .filter(|k| !registered.contains(k))
        .collect();
    let orphan: Vec<&&str> = registered
        .iter()
        .filter(|k| !kinds::ALL.contains(k))
        .collect();
    // A duplicate row would let one kind's condition stand in for another's.
    let mut seen = registered.clone();
    seen.sort_unstable();
    let dupes = seen.windows(2).any(|w| w.first() == w.last());
    Probe {
        name: "audit: every attention kind has a registry row, and every registered condition is one the code emits",
        red_fires: !kinds::ALL.is_empty() && !registered.is_empty(),
        green_passes: unregistered.is_empty() && orphan.is_empty() && !dupes,
    }
}

fn all_probes() -> Vec<Probe> {
    vec![
        probe_every_condition_kind_is_registered(),
        probe_model_is_recorded_per_session(),
        probe_lease_store_is_named(),
        probe_no_task_no_prompt(),
        probe_gate_verify(),
        probe_gate_main(),
        probe_session_identity_is_the_launchers(),
        probe_shipped_advice_names_real_subcommands(),
        probe_status_tests_hold_one_instant(),
        probe_gate_names_the_landing_that_moved_main(),
        probe_handover_ok_names_the_main_it_checked(),
        probe_handover_matcher(),
        probe_ledger_roundtrip(),
        probe_green_follows_the_tree_only_where_declared(),
        probe_killed_is_no_verdict(),
        probe_git_ancestor(),
        probe_gate_claim(),
        probe_claim_cas(),
        probe_attention(),
        probe_channel_dedupe(),
        probe_install_merge(),
        probe_gate_digest(),
        probe_lease_take(),
        probe_launch_no_tty(),
        probe_worker_task_prompt(),
        probe_stop_nudge(),
        probe_nudge_names_only_claimable(),
        probe_standstill(),
        probe_idle_without_claim_needs_a_live_session(),
        probe_expired_cutoff_is_reported(),
        probe_close_releases_the_claim(),
        probe_handover_not_green_is_one_line_per_worker(),
        probe_status_bd_budget_follows_the_measurement(),
        probe_agent_traffic_is_counted(),
        probe_a_message_is_recorded_with_its_content(),
        probe_owner_queue_is_the_ready_line_not_a_condition(),
        probe_handover_names_the_held_bead_and_skips_with_none(),
        probe_a_superseding_branch_hands_over_by_its_trailer(),
        probe_ready_split_names_epics_apart(),
        probe_holdings_tags_name_their_tense(),
        probe_install_reports_a_stale_bd_prime_hook(),
        probe_batch_ready_is_a_fact_with_three_parts(),
        probe_lease_defect_reaches_the_waiter(),
        probe_yesterdays_repo_is_told_and_a_current_one_is_not(),
        probe_install_goes_forward_only(),
        probe_enforced_gate(),
        probe_env_reaches_the_hook(),
        probe_worktree_is_airs(),
        probe_digest_refusal_names_the_order_only_with_a_green(),
        probe_install_refuses_unignored_air(),
        probe_bd_calls_are_per_event(),
        probe_status_reconcile_is_one_show(),
        probe_subagent_stop_is_not_a_stop(),
        probe_batch_close(),
        probe_triage_bead_exists(),
        probe_surface_diff(),
        probe_change_only_push(),
        probe_conditions_logged_on_change_only(),
        probe_doctor_enumerates_tables(),
        probe_gc_keeps_what_it_must(),
        probe_peer_warning_effect_is_readable(),
        probe_poll_tick_pays_for_bd_rarely(),
        probe_registry_traces_are_unambiguous(),
        probe_coordinator_may_commit_never_push(),
        probe_bead_attribution_reads_a_trailer(),
        probe_digest_names_its_bead(),
        probe_land_selection_is_never_silent(),
        probe_audit_registry(),
        probe_audit_unregistered_firing(),
        probe_land_refusals(),
        probe_project_is_taken_from_what_it_is_told(),
        probe_audit_help_names_only_what_it_prints(),
        probe_landed_but_open(),
        probe_refused_landing_publishes_nothing(),
        probe_land_by_bead_carries_the_whole_branch(),
        probe_acceptance_unread_refuses(),
        probe_claim_retries_a_timeout_once(),
        probe_release_unassigns(),
        probe_every_air_spawn_pins_identity(),
        probe_worker_cannot_ask_the_owner_directly(),
        probe_batch_green_closes_the_bead_it_covers(),
        probe_red_batch_is_reported_by_member_and_lands_nothing(),
        probe_install_lag_is_named(),
        probe_no_session_reads_stuck(),
        probe_hook_reads_from_the_worktree_root(),
        probe_acceptance_budget_scales_with_ids(),
        probe_contradicts_names_only_the_refuted(),
        probe_unresolvable_path_is_unreadable_not_refuted(),
        probe_land_names_a_branch(),
        probe_close_with_proof_sequence(),
        probe_verify_in_flight(),
        probe_landing_state(),
        probe_land_role_is_where_you_are(),
        probe_landable_pushes_once_per_branch(),
        probe_nothing_unverified_reaches_main(),
        probe_air_runs_no_conflicting_merge(),
        probe_idle_without_claim_counts_claimable_only(),
        probe_every_wait_is_recorded_once_against_its_own_budget(),
        probe_every_budget_has_a_catalogue_row_naming_its_fail_direction(),
        probe_a_hook_records_its_own_wall_clock(),
        probe_an_unpaired_hook_is_counted_from_the_installed_matchers(),
        probe_a_notice_waits_for_the_round_and_the_release_refuses(),
        probe_an_edit_outside_the_worktree_is_denied(),
        probe_metis_is_the_coordinators_and_never_a_workers(),
        probe_an_initiative_is_declared_and_counted_without_a_gate(),
        probe_no_tracked_file_names_an_adopter(),
        probe_every_declared_mutation_still_anchors(),
    ]
}

/// air-s7c: `review-waiting` and `owner-decision-waiting` became change-only pushes. Red: the
/// same set evaluated twice pushes once — the repeat is suppressed, and a merely older
/// condition is still a repeat. Green: a real change (a bead joins the waiting set, the owner
/// queue depth moves) pushes again. peer-warning-repeat was deleted for allegedly failing to
/// suppress, so this proves the suppression suppresses.
fn probe_change_only_push() -> Probe {
    use crate::cmd::mcp::{Pushed, select_new};
    use crate::cmd::status::Attention;

    let review = |bead: &str, mins: i64| Attention {
        worker: bead.to_string(),
        kind: "review-waiting",
        detail: format!("{bead} handed over {mins} min ago"),
        for_minutes: mins,
        fingerprint: format!("{bead}/alpha"),
    };
    let queue = |attempts: usize, mins: i64| Attention {
        worker: "beta".to_string(),
        kind: "handover-not-green",
        detail: format!("{attempts} attempt(s), oldest {mins} min"),
        for_minutes: mins,
        fingerprint: format!("attempts:{attempts}"),
    };

    let mut pushed = Pushed::new();
    // First evaluation: both are new, both push.
    let first = select_new(&mut pushed, &[review("air-1", 5), queue(2, 5)]);
    // Same facts, much later: age is not a change, so nothing is pushed. Under the old
    // doubling rule 5 -> 40 min would have re-pushed both.
    let same_again = select_new(&mut pushed, &[review("air-1", 40), queue(2, 40)]);
    let red = first.len() == 2 && same_again.is_empty();

    // A bead joins the set, and the attempt count moves: both are real changes.
    let changed = select_new(
        &mut pushed,
        &[review("air-1", 45), review("air-2", 1), queue(3, 45)],
    );
    let green = changed.len() == 2
        && changed.iter().any(|a| a.worker == "air-2")
        && changed.iter().any(|a| a.worker == "beta")
        // ...and the unchanged bead did NOT ride along with them.
        && !changed.iter().any(|a| a.worker == "air-1");
    Probe {
        name: "channel: an unchanged set pushes once however old it gets; a changed set pushes again",
        red_fires: red,
        green_passes: green,
    }
}

/// air-5uz: the poll writes the condition set to the event log on change only.
///
/// The channel re-evaluates every few seconds. On 2026-08-25 that put 7,667 of the day's
/// 8,242 event lines in the log, and `air audit` counted them as firings: 1,685 for
/// `owner-decision-waiting`, which was sent once. A deletion was nearly proposed on that
/// number. Nothing is lost by the silence, because the `conditions` table already carries
/// first-seen, last-seen and cleared for every condition.
///
/// Red: sixty ticks of an unchanged set write ONE line, not sixty. Green: the set changing
/// writes again, so the log still says when something happened.
fn probe_conditions_logged_on_change_only() -> Probe {
    use crate::cmd::status::{Attention, Snapshot, record_and_log};

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let l = Ledger::open_in(&dir).map_err(|e| e.to_string())?;
        let events = dir
            .join("events")
            .join(format!("{}.ndjson", crate::cmd::today()));
        let lines = |p: &Path| {
            std::fs::read_to_string(p)
                .map(|t| t.lines().count())
                .unwrap_or(0)
        };
        let waiting = |mins: i64| Attention {
            worker: "beta".to_string(),
            kind: "handover-not-green",
            detail: format!("4 attempts, oldest {mins} min"),
            for_minutes: mins,
            fingerprint: "attempts:4".to_string(),
        };

        // An hour of polling with nothing changing but the clock.
        for tick in 0..60 {
            let snap = Snapshot {
                at: format!("2026-08-25T10:{tick:02}:00Z"),
                ..Default::default()
            };
            record_and_log(&l, "main", &snap, &[waiting(tick)], true);
        }
        let red = lines(&events) == 1;

        // A fifth attempt: a real change, said again.
        let snap = Snapshot {
            at: "2026-08-25T11:00:00Z".to_string(),
            ..Default::default()
        };
        let changed = Attention {
            fingerprint: "attempts:5".to_string(),
            ..waiting(61)
        };
        record_and_log(&l, "main", &snap, &[changed], true);
        let green = lines(&events) == 2;

        std::fs::remove_dir_all(&dir).ok();
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "status: an unchanged condition set writes one event line an hour, not one a tick",
        red_fires: red,
        green_passes: green,
    }
}

/// air-w0e: `air doctor` counts every table the ledger has, asked of `sqlite_master`.
///
/// It used to walk a list somebody typed, and reported 7 of the 11 tables at schema v10:
/// `hook_emissions`, `conditions`, `lease_wants` and `bd_cache` were invisible, which is how
/// the zero-lease finding nearly went unnoticed. Currency, not presence.
///
/// Red: the four tables the list left out are all counted. Green: a table this probe invents,
/// which no list anywhere could name, is counted too — so the next migration needs no edit
/// here.
fn probe_doctor_enumerates_tables() -> Probe {
    use crate::cmd::doctor::table_rows;

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let named = |rows: &[(String, i64)], t: &str| rows.iter().any(|(n, _)| n == t);

        let rows = table_rows(l.conn());
        let red = ["hook_emissions", "conditions", "lease_wants", "bd_cache"]
            .iter()
            .all(|t| named(&rows, t));

        l.conn()
            .execute_batch("CREATE TABLE a_table_no_list_could_name (x INTEGER)")
            .map_err(|e| e.to_string())?;
        let rows = table_rows(l.conn());
        let green = rows
            .iter()
            .any(|(n, c)| n == "a_table_no_list_could_name" && *c == 0);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "doctor: every table the ledger has is counted, including one added after this probe was written",
        red_fires: red,
        green_passes: green,
    }
}

/// air-i7s: `air gc` keeps what it must and removes only what it may.
///
/// The stream is the only artefact that has caught the audit's own errors (0007 §11), so this
/// probe is about the failure DIRECTION: a collector that errs must err toward keeping.
///
/// Red: an old day the ledger still points at is kept, and so is a day inside the window, and
/// an unreadable clock keeps everything rather than collecting everything. Green: an old day
/// nothing points at is the one thing collected, and its bytes are the reported total.
fn probe_gc_keeps_what_it_must() -> Probe {
    use crate::cmd::gc::{plan, referenced_days};

    let days = [
        ("2026-01-01".to_string(), 100u64), // old, unreferenced -> collect
        ("2026-01-02".to_string(), 200u64), // old, but a landing sits in it -> keep
        ("2026-08-29".to_string(), 400u64), // inside the window -> keep
    ];
    let referenced: std::collections::BTreeSet<String> =
        std::iter::once("2026-01-02".to_string()).collect();

    let p = plan(&days, "2026-08-29", 90, &referenced);
    let kept = |i: usize| p.days.get(i).and_then(|d| d.kept);
    let red = kept(1) == Some("the ledger still points at this day")
        && kept(2) == Some("inside the retention window")
        // A clock it cannot read keeps everything. The other direction deletes the record.
        && plan(&days, "not-a-date", 90, &Default::default()).collectable_bytes == 0;

    let green = kept(0).is_none()
        && p.collectable_bytes == 100
        && p.total_bytes == 700
        // Nothing is removed by planning, and `applied` says so.
        && !p.applied;

    // And the referenced set is read from the ledger, not from a list: a landing written now
    // protects its own day.
    let live = (|| -> Result<bool, String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.conn()
            .execute(
                "INSERT INTO landings (id, worker, sha, result, attempt_no, started_at, finished_at) \
                 VALUES ('x','alpha','deadbeef','landed',1,'2026-01-02T10:00:00Z','2026-01-02T10:05:00Z')",
                [],
            )
            .map_err(|e| e.to_string())?;
        Ok(referenced_days(l.conn()).contains("2026-01-02"))
    })()
    .unwrap_or(false);

    Probe {
        name: "gc: an old day the ledger points at is kept, an unreadable clock keeps everything, only an unreferenced old day is collected",
        red_fires: red && live,
        green_passes: green,
    }
}

/// air-1ra: whether a peer warning changed what the worker did is readable from the record.
///
/// `peer-warning` had 33 firings and no demonstrated effect in either direction, and the
/// absence of recorded harm was partly because the effect was not recorded. It was: a `warn`
/// line carries `session_id` and `path`, and so does every `journaled` line. No new recording
/// was added for this.
///
/// Red: warn, then the session edits that file twice more, and it reads IGNORED. Green: warn,
/// then only the edit already in flight, and it reads heeded — and edits by ANOTHER session,
/// or to another file, do not count against it, which is the join being a join.
fn probe_peer_warning_effect_is_readable() -> Probe {
    use crate::cmd::audit::peer_effect;

    let w = |at: &str, sid: &str, path: &str| {
        (
            at.to_string(),
            "alpha".to_string(),
            sid.to_string(),
            path.to_string(),
        )
    };
    let e = |at: &str, sid: &str, path: &str| (at.to_string(), sid.to_string(), path.to_string());

    let warns = [w("10:00", "s1", "a.rs"), w("10:00", "s2", "b.rs")];
    let edits = [
        // s1 was warned about a.rs and kept going: the in-flight edit plus two more.
        e("10:01", "s1", "a.rs"),
        e("10:02", "s1", "a.rs"),
        e("10:03", "s1", "a.rs"),
        // s2 was warned about b.rs and stopped after the edit already in flight.
        e("10:01", "s2", "b.rs"),
        // Noise that must not count: another session in the same file, the same session in
        // another file, and an edit BEFORE the warning.
        e("10:05", "s9", "b.rs"),
        e("10:05", "s2", "c.rs"),
        e("09:00", "s2", "b.rs"),
    ];

    let p = peer_effect(&warns, &edits);
    let row = |i: usize| p.warned.get(i);
    let red = p.warnings == 2
        && p.ignored == 1
        && row(0).is_some_and(|r| r.edits_after == 3 && !r.heeded);
    let green = p.heeded == 1
        && row(1).is_some_and(|r| r.edits_after == 1 && r.heeded)
        // Never guessed at: a conflict count nobody records is reported as unrecorded, not 0.
        && p.conflicts_in_warned_files.is_none();
    Probe {
        name: "audit: a warned session that keeps editing the file reads IGNORED; one that stops reads heeded",
        red_fires: red,
        green_passes: green,
    }
}

/// air-cmn: an ordinary poll tick answers from the cache and never shells out to bd.
///
/// The poll ran a full `gather` every ~8 s and every one called bd — `in_progress`, then `show`
/// once per open claim, then `awaiting_review`, then `ready`: about 5,700 bd calls and 2.3
/// hours a day waiting on bd, to deliver ~45 pushes (0007 §3). air-djl proposed deleting the
/// thread over the event volume, which air-5uz had already removed; the cost was here.
///
/// This asserts the DECISION, not a process count, because the fallback it arms is the
/// already-tested slow-bd path: `cache_is_fresh` says whether this tick pays.
///
/// Red: with a cached answer a minute old and a 10-minute window, the tick uses the cache.
/// Green: an answer older than the window, and an empty cache after a restart, both pay — so
/// the counts cannot go stale forever and the first tick still fills the cache.
fn probe_poll_tick_pays_for_bd_rarely() -> Probe {
    use crate::cmd::status::cache_is_fresh;

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let now = "2026-08-29T12:00:00Z";

        // Nothing cached: a restart must pay once rather than answer from nothing.
        let empty_pays = !cache_is_fresh(&l, now, 10);

        l.bd_cache_put("ready_depth", "21", "2026-08-29T11:59:00Z")
            .map_err(|e| e.to_string())?;
        let red = cache_is_fresh(&l, now, 10);

        // The same value, an hour old: outside the window, so this tick pays.
        l.bd_cache_put("ready_depth", "21", "2026-08-29T11:00:00Z")
            .map_err(|e| e.to_string())?;
        let green = !cache_is_fresh(&l, now, 10) && empty_pays;
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "status: a poll tick with fresh cached counts calls bd not at all; a stale or empty cache pays once",
        red_fires: red,
        green_passes: green,
    }
}

/// air-iy1: the coordinator's boundary is the remote, not main.
///
/// The incident: the coordinator wrote plan 0008, a decisions entry and a CLAUDE.md index row,
/// could not commit them, and the owner committed by hand — the owner doing a chore the
/// coordinator was in the middle of. `air land` already merges into main and is already the
/// coordinator's, so the deny was never protecting main; it stopped the coordinator saving its
/// own prose. Owner ruling, 2026-08-29.
///
/// Asserted on the ARGV the launcher builds, which is where the rule lives — the deny is passed
/// to `claude --disallowed-tools` at launch. Note what this probe does NOT claim: a coordinator
/// session already running keeps the flags it started with until it is relaunched against a
/// rebuilt binary. "The rule is changed" and "that session can commit" are different claims.
///
/// Red: `git push` is still denied, for the coordinator and the worker both. Green: `git commit`
/// is denied for neither role — a worker's commits are the whole point of a worktree, and the
/// coordinator's own prose is its own to save.
fn probe_coordinator_may_commit_never_push() -> Probe {
    use crate::cmd::launch::{coordinator_argv, worker_argv};

    let coord = coordinator_argv("air", Path::new("/r/.air/roles.md"), "--channels", &[], &[]);
    let worker = worker_argv("w", "air", Path::new("/r/.air/roles.md"), &[]);
    let denies = |v: &[String], pat: &str| v.iter().any(|a| a == pat);

    let red = denies(&coord, "Bash(git push *)") && denies(&worker, "Bash(git push *)");
    let green = !denies(&coord, "Bash(git commit *)") && !denies(&worker, "Bash(git commit *)");
    Probe {
        name: "launch: neither role may push; neither is denied `git commit` (the boundary is the remote, not main)",
        red_fires: red,
        green_passes: green,
    }
}

/// air-8br: no `command / decision` pair is claimed by two mechanisms, and the pairs the audit
/// treats as bookkeeping are not also claimed as firings.
///
/// The registry is the audit's only map of what Air ships. Two rows claiming one trace would
/// count every firing twice and split it across two removal conditions, and a trace that is
/// both registered and bookkeeping would be attributed and suppressed at once. Neither is
/// visible in the output: the numbers would simply be wrong, which is air-5uz's failure shape.
///
/// Red: every registered trace is claimed exactly once. Green: no registered trace is also in
/// the bookkeeping list, and the registry is not empty — a check that passes on nothing is the
/// anti-pattern this whole command exists against.
fn probe_registry_traces_are_unambiguous() -> Probe {
    use crate::cmd::audit::{BOOKKEEPING, registered_traces};
    use crate::cmd::mechanisms::{Fires, MECHANISMS};

    let mut all: Vec<String> = Vec::new();
    for m in MECHANISMS {
        if let Fires::Decisions(traces) = m.fires {
            all.extend(traces.iter().map(|(c, d)| format!("{c} / {d}")));
        }
    }
    let distinct = registered_traces();
    let red = !all.is_empty() && all.len() == distinct.len();

    // A decision word cannot be both a mechanism firing and bookkeeping.
    let green = !distinct.is_empty()
        && !all.iter().any(|t| {
            t.split(" / ")
                .nth(1)
                .is_some_and(|d| BOOKKEEPING.contains(&d))
        });
    Probe {
        name: "audit: every registered trace is claimed by exactly one mechanism and none is also bookkeeping",
        red_fires: red,
        green_passes: green,
    }
}

/// air-6u5: selection never answers "nothing" when it means "something broke", and a bead
/// stays attributed after the branch merges `main`.
///
/// Red, the bug that made `air land` unusable in the adopter: the old narrowing required a claim
/// `claimed_at >= branch_point`, and merging `main` moves the branch point FORWARD past the
/// claim that started the work — while landing requires merging main. So preparing to land
/// destroyed the attribution. Here the claim is older than the branch point, as it is for
/// every real branch that has merged main, and it must still be attributed.
///
/// Green: a ledger error is an error, not an empty queue. `{"landed": [], "ok": true}` was the
/// worst answer available because there was nothing to disbelieve.
fn probe_land_selection_is_never_silent() -> Probe {
    use crate::cmd::status::Skipped;

    // The narrowing that broke it is gone: what remains is claimed-by-this-worker and
    // not-already-landed, neither of which moves when git does.
    let red = (|| -> Result<bool, String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        // Claimed BEFORE the branch point, which is what merging main produces.
        l.record_claim("zz-1", "alpha", &[], "2026-08-22T10:00:00Z")
            .map_err(|e| e.to_string())?;
        let kept = crate::cmd::status::attributable_for_test(&l, &["zz-1".to_string()], "alpha")?;
        // ...and a bead already landed is dropped, which is the bound that replaced the time.
        let dropped =
            crate::cmd::status::attributable_for_test(&l, &["zz-other".to_string()], "alpha")?;
        Ok(kept == ["zz-1"] && dropped.is_empty())
    })()
    .unwrap_or(false);

    // A Skipped row carries the check name and the fixing command, so nothing is ever a bare
    // absence.
    let sk = Skipped {
        worker: "alpha".to_string(),
        check: "green-at-head",
        detail: "alpha has no recorded green at its head abc12345".to_string(),
        fix: "in that worktree: air record verify -- <the repo's verify>".to_string(),
    };
    let green = !sk.fix.is_empty() && !sk.detail.is_empty() && sk.check == "green-at-head";
    Probe {
        name: "land: a claim older than the branch point still attributes; every skip names its check and fix",
        red_fires: red,
        green_passes: green,
    }
}

/// air-agq: the digest gate reads a declared `bead:` field instead of guessing from a
/// filename and an mtime.
///
/// It GUARDS, so every way it used to be wrong failed toward permitting, and a missing refusal
/// looks exactly like a satisfied one. Red covers the three ways it passed when it should not:
/// a digest for a different bead, a digest touched rather than written, and a file that merely
/// has the worker's name in it. Green: the digest that declares this bead is accepted, and a
/// pre-cutoff digest with no front matter still passes so today's work is not invalidated.
fn probe_digest_names_its_bead() -> Probe {
    use crate::cmd::handover::{declared_bead, digest_for_bead};

    let res = (|| -> Option<(bool, bool)> {
        let root = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&root).ok()?;
        let dir = root.as_path();
        // Strict: every file here counts as written after the cutoff, so only a declaration
        // is accepted. The lenient cutoff below is the history case.
        let cut: jiff::Timestamp = "2000-01-01T00:00:00Z".parse().ok()?;
        let write = |name: &str, body: &str| std::fs::write(dir.join(name), body).ok();
        // A digest for ANOTHER bead, by this worker, written now.
        write(
            "2026-08-23-beta-air-other.md",
            "---\nbead: air-other\n---\n# other\n",
        )?;
        let ours = vec!["air-agq".to_string()];

        // Red: it declares a different bead, so it is not this bead's digest, whatever its
        // name or mtime say.
        let wrong_bead = !digest_for_bead(dir, "beta", &ours, None, cut);
        // Red: a file carrying the worker's name and no declaration, written after the
        // cutoff, is not a substitute — this is the `touch` case and the substring case.
        write("2026-08-23-beta-notes.md", "# just some notes\n")?;
        let undeclared_after_cutoff = !digest_for_bead(dir, "beta", &ours, None, cut);
        let red =
            wrong_bead && undeclared_after_cutoff && declared_bead("# no front matter").is_none();

        // Green: the digest that declares this bead is accepted.
        write(
            "2026-08-23-beta-air-agq.md",
            "---\nbead: air-agq\n---\n# ours\n",
        )?;
        let declared_ok = digest_for_bead(dir, "beta", &ours, None, cut);

        // Green: history still passes. A digest written before the cutoff with no front
        // matter is matched the old way, so the change does not invalidate what exists.
        let old = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&old).ok()?;
        std::fs::write(old.join("2026-08-22-beta-air-old.md"), "# old\n").ok()?;
        let far_future: jiff::Timestamp = "2999-01-01T00:00:00Z".parse().ok()?;
        let fallback_ok = digest_for_bead(&old, "beta", &ours, None, far_future);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&old);
        Some((red, declared_ok && fallback_ok))
    })();
    // An Option, not a Result: this fixture has no error text to carry.
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "gate: a digest counts when it declares its bead; a different bead, a touch, or a name match do not",
        red_fires: red,
        green_passes: green,
    }
}

/// air-4re: which beads a branch carries is written by a machine and read by a machine.
///
/// Red: a commit whose body MENTIONS three beads and carries one `Bead:` trailer is attributed
/// to one — the case the prose scan cannot get right, because a mention and an attribution
/// have identical grammar. air-7kp needed an authorship filter and a branch-point bound on top
/// of the scan and still went 8 → 6 → 3 against one real branch.
///
/// Green: the dated fallback still reads history (a commit before the cutoff with no trailer),
/// and a commit after it with no trailer is attributed to nothing rather than guessed at.
fn probe_bead_attribution_reads_a_trailer() -> Probe {
    use crate::cmd::attribution::{Commit, cutoff, ids_of, prose_ids, trailer_ids};

    let mentions = "fix(land): select from the merge range\n\nBuilds on air-3pz, measured in \
                    air-869, supersedes air-7kp.\n\nBead: air-4re\n";
    let red = trailer_ids(mentions) == ["air-4re"]
        && ids_of(
            &[Commit {
                committed: "2026-08-22T10:00:00Z".to_string(),
                message: mentions.to_string(),
            }],
            prose_ids,
            cutoff(),
        ) == ["air-4re"]
        // ...and the scan on its own would indeed have taken all four.
        && prose_ids(mentions).len() >= 4;

    let old = Commit {
        committed: "2020-01-01T00:00:00Z".to_string(),
        message: "fix: the work (air-old)\n".to_string(),
    };
    let new = Commit {
        committed: "2999-01-01T00:00:00Z".to_string(),
        message: "fix: the work (air-new)\n".to_string(),
    };
    let green = ids_of(&[old], prose_ids, cutoff()) == ["air-old"]
        && ids_of(&[new], prose_ids, cutoff()).is_empty();
    Probe {
        name: "land: a `Bead:` trailer attributes the commit; a mention does not; the prose fallback is dated",
        red_fires: red,
        green_passes: green,
    }
}

/// air-zyo: the registry's job is that a mechanism nobody wrote a removal condition for is
/// visible. Red: an entry with nothing recorded is reported as a defect. Green: an entry
/// with a condition is not, and its counter reads back.
fn probe_audit_registry() -> Probe {
    use crate::cmd::audit::{NO_CONDITION, gather_from, removal_verdict};
    use crate::cmd::mechanisms::Removal;

    let events = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"main","command":"status.attention","inputs":{"conditions":["handover-not-green:alpha"]},"decision":"attention"}"#,
        "\n",
    );
    let a = gather_from(
        &[("2026-08-22".to_string(), events.to_string())],
        "2026-08-22",
    );
    // Red: a mechanism with nothing recorded IS reported as a defect. Asserted against the
    // classifier rather than against a registry row that happens to lack a condition — this
    // probe pointed at `review-waiting` until air-s7c gave that one a condition, then at
    // `stuck` until air-byw gave `stuck` one (air-dqw's deletion was reverted on that finding).
    // Each time, the probe went silent on a registry change that was not a regression. There is
    // now no `Removal::Unstated` row left, which is the goal, so a probe that needs one would be
    // a probe that needs a defect to exist.
    //
    // Two lanes reached this same fix independently within the hour; this is main's version,
    // which asserts the whole verdict tuple rather than only the defect string.
    let red = removal_verdict(Removal::Unstated, 0) == ("none", None, Some(NO_CONDITION))
        && a.rows.iter().all(|r| r.defect.is_none());
    // Green: a mechanism that does carry one is not a defect, and the counter works.
    let green = a
        .rows
        .iter()
        .any(|r| r.id == "idle-without-claim" && r.defect.is_none())
        && a.rows
            .iter()
            .any(|r| r.id == "handover-not-green" && r.evaluations == 1 && r.last_fired.is_some());
    Probe {
        name: "audit: a mechanism with no recorded removal condition is a defect; one with a condition counts",
        red_fires: red,
        green_passes: green,
    }
}

/// air-2zq: close-with-proof, end to end. air-i59 made the gate blocking on evidence measured
/// against the hand-over flow; air-7o3 replaced hand-over with closing, and the question raised
/// was whether the gate now bills a fresh verify per bead and whether an agent can stall.
///
/// Red: once HEAD moves, the next close IS refused until a verify is recorded there — the gate
/// still bites, which is the half worth keeping. Green: claim, close, take the next bead, close
/// again on an UNCHANGED HEAD, all on one verify run. So the second close is free and the
/// sequence in CLAUDE.md's work flow does not stall.
fn probe_close_with_proof_sequence() -> Probe {
    use crate::cmd::hook::handover_gate;
    use air_hooks::HookOutcome;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let head = g(&["rev-parse", "HEAD"])?;

        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let green_at = |sha: &str| -> Result<(), String> {
            l.record_verify(&VerifyRun {
                id: new_id(),
                worker: "probe".into(),
                sha: sha.to_string(),
                kind: Kind::Verify,
                exit_code: 0,
                trigger: "selftest".into(),
                failing_step: None,
                started_at: "t".into(),
                finished_at: "t".into(),
                log_path: None,
                command: None,
                duration_ms: None,
                output_bytes: None,
                dirty: false,
                tree: None,
                members: vec![],
            })
            .map_err(|e| e.to_string())
        };
        let passes = |bead: &str| -> bool {
            matches!(
                handover_gate(&l, "probe", &dir, &format!("bd close {bead}"), true)
                    .map(|d| d.outcome),
                Ok(HookOutcome::Allow { .. })
            )
        };

        // Bead one: claim, work already committed, verify recorded, close.
        l.record_claim("zz-1", "probe", &[], "t0")
            .map_err(|e| e.to_string())?;
        green_at(&head)?;
        let first = passes("zz-1");

        // Bead two, finished without moving HEAD (a docs bead already satisfied, a no-op fix).
        // ONE verify run exists in total, and this close must still pass: a green at a commit
        // that has not moved is still a green.
        l.record_claim("zz-2", "probe", &[], "t1")
            .map_err(|e| e.to_string())?;
        let second_free = passes("zz-2");
        let runs: i64 = l
            .conn()
            .query_row("SELECT count(*) FROM verify_runs", [], |r| r.get(0))
            .unwrap_or(-1);

        // Bead three, with a commit: HEAD moved, so the gate demands a verify there.
        g(&["commit", "-q", "--allow-empty", "-m", "b"])?;
        l.record_claim("zz-3", "probe", &[], "t2")
            .map_err(|e| e.to_string())?;
        let refused_after_commit = !passes("zz-3");
        // ...and recording one at the new HEAD clears it. No stall.
        let moved = g(&["rev-parse", "HEAD"])?;
        green_at(&moved)?;
        let cleared = passes("zz-3");

        Ok((
            refused_after_commit,
            first && second_free && runs == 1 && cleared,
        ))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "gate: two closes on one unchanged HEAD cost one verify; a commit demands a new one and clears",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-ha8: `air audit --help` advertised "how often with nothing following" for a round after
/// the owner cut that metric — a derived statement reading as an observed one, in the help of
/// the command built to surface exactly that. The check is the containment: every field the
/// help names in backticks must appear in what the command prints.
///
/// Red: a help text that names one more field than the command prints is caught. Green: the
/// real help text passes.
fn probe_audit_help_names_only_what_it_prints() -> Probe {
    use crate::cmd::audit::{gather_from, render};
    use clap::CommandFactory;

    // Backticked names are the contract: prose around them is free, the names are checked.
    fn named(help: &str) -> Vec<String> {
        help.split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    }
    let help = crate::Cli::command()
        .find_subcommand("audit")
        .and_then(|c| c.get_long_about().or_else(|| c.get_about()).cloned())
        .map(|s| s.to_string())
        .unwrap_or_default();
    // One registered mechanism firing, so a row with a removal condition renders in full.
    let events = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"main","command":"status.attention","inputs":{"conditions":["review-waiting:air-1"]},"decision":"attention"}"#,
        "\n",
    );
    let printed = render(&gather_from(
        &[("2026-08-22".to_string(), events.to_string())],
        "2026-08-22",
    ));
    let all_printed = |h: &str| {
        let names = named(h);
        !names.is_empty() && names.iter().all(|n| printed.contains(n.as_str()))
    };
    Probe {
        name: "audit: every field the help names in backticks is one the command prints",
        red_fires: !all_printed(&format!("{help} and `how often with nothing following`")),
        green_passes: all_printed(&help),
    }
}

/// air-ayp: `air land` closes nothing (the worker closes its own bead with proof), so what a
/// landing carries past its print is the one signal meaning a wrong close: a clause the merge
/// CONTRADICTS. As a `landings` row, never as a bd status.
///
/// Red: a bead naming a file the merge did not touch is refuted, not discharged, the ledger
/// reports it with the clause, and the report SURVIVES the claim being claimed and released —
/// which is what air-dlw fixed, because close-with-proof reconciles the claim away at once and
/// a report keyed on it could never fire. Green: a bead whose every clause is discharged says
/// so; one Air merely cannot read is neither refuted nor discharged and is not reported; and a
/// later landing that stops refuting the bead clears it.
///
/// The no-blocking half is the second assertion: the whole representation is a ledger row, and
/// the bead's bd status is untouched, so a dependent is exactly as blocked as it was before
/// the merge. bd's blocking predicate never consults the workflow class, which is why parking
/// it in a done-class status would have blocked dependents indefinitely.
fn probe_landed_but_open() -> Probe {
    use crate::cmd::acceptance::{Evidence, judge_clauses};
    use air_ledger::landings::{Landing, OpenBead};

    let changed = vec!["docs/rules/roles.md".to_string()];
    let tree = vec![
        "docs/rules/roles.md".to_string(),
        "docs/rules/writing.md".to_string(),
        "docs/absent.md".to_string(),
    ];
    let ev = Evidence {
        green_at_landed: true,
        changed: &changed,
        tree: &tree,
    };
    // A clause the merge CONTRADICTS: the bead names a file it did not touch.
    let refutable = judge_clauses(
        "zz-2",
        vec!["docs/rules/writing.md names the rule.".into()],
        &ev,
    );
    // A clause Air simply cannot read. Not a defect, and not the wrong-close signal.
    let unreadable = judge_clauses(
        "zz-3",
        vec!["The owner rules on the counter-argument.".into()],
        &ev,
    );
    let discharged = judge_clauses(
        "zz-1",
        vec![
            "Verify recorded green at HEAD.".into(),
            "docs/rules/roles.md names the rule.".into(),
        ],
        &ev,
    );

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_landing(&Landing {
            despite_inflight: vec![],
            members: vec![],
            id: new_id(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: "landed-refuted".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 1,
            beads: vec!["zz-1".into(), "zz-2".into()],
            open_beads: vec![OpenBead {
                bead: "zz-2".into(),
                why: refutable.why_open(),
                refuted: true,
                contradicted: refutable.why_contradicted(),
            }],
            merge_commit: Some("ccc".into()),
            pid: None,
            started_at: "t0".into(),
            finished_at: "t1".into(),
        })
        .map_err(|e| e.to_string())?;
        let open = l.landed_open().map_err(|e| e.to_string())?;
        // Reported, with the clause, and the closable bead is NOT in the held-open set.
        let reported = open.len() == 1
            && open
                .first()
                .is_some_and(|o| o.bead == "zz-2" && o.why.contains("docs/rules/writing.md"));
        // air-dlw: the claim's lifetime must NOT decide this. Under close-with-proof the
        // worker closes at once and the reconcile releases the claim on the next tick, so a
        // report keyed on the claim could never fire. Claim it, release it as the reconcile
        // does, and the report has to survive both.
        l.record_claim("zz-2", "alpha", &[], "t2")
            .map_err(|e| e.to_string())?;
        l.release_claims_on(&["zz-2".to_string()], "closed", "t3")
            .map_err(|e| e.to_string())?;
        let survives_the_claim = l.landed_open().map_err(|e| e.to_string())?.len() == 1;

        // It clears when a LATER landing of the same bead stops refuting it.
        l.record_landing(&Landing {
            despite_inflight: vec![],
            members: vec![],
            id: new_id(),
            worker: "alpha".into(),
            sha: "ddd".into(),
            tip_sha: Some("ccc".into()),
            result: "landed".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 2,
            beads: vec!["zz-2".into()],
            open_beads: vec![OpenBead {
                bead: "zz-2".into(),
                why: "a clause Air cannot read".into(),
                refuted: false,
                contradicted: String::new(),
            }],
            merge_commit: Some("eee".into()),
            pid: None,
            started_at: "t4".into(),
            finished_at: "t5".into(),
        })
        .map_err(|e| e.to_string())?;
        let cleared = l.landed_open().map_err(|e| e.to_string())?.is_empty();
        Ok((reported && survives_the_claim, cleared))
    })()
    .unwrap_or_else(blocked);
    let (reported, cleared) = res;

    Probe {
        name: "land: a contradicted clause is reported from the landing and survives the claim being reconciled away; one Air cannot read is not",
        red_fires: refutable.refuted() && !refutable.all_discharged() && reported,
        green_passes: discharged.all_discharged()
            && !unreadable.refuted()
            && !unreadable.all_discharged()
            && cleared,
    }
}

/// air-8zn: a REFUSED landing publishes no landed beads, and a successful one still publishes
/// all of them. `landed_open` excluded only `in-flight` and treated every other result as a
/// landing, so the second branch of an `air land --all` — refused for main-moved, as every
/// branch after the first is — was read as the newest word on the five beads its row carried.
///
/// Red: a refused row carrying a refuted bead reports nothing, and a refused row NEWER than a
/// real landing does not silence that landing's refutation. Green: the real landing reports
/// every refuted bead it carries, which is what stops the fix becoming a silence.
fn probe_refused_landing_publishes_nothing() -> Probe {
    use air_ledger::landings::{Landing, OpenBead};

    let row = |id: &str, result: &str, at: &str, beads: &[&str], open: Vec<OpenBead>| Landing {
        despite_inflight: vec![],
        members: vec![],
        id: id.into(),
        worker: "w4".into(),
        sha: "823b2fd5".into(),
        tip_sha: Some("99b10fa0".into()),
        result: result.into(),
        failing_step: (result == "refused").then(|| "check".to_string()),
        verify_run_id: None,
        attempt_no: 1,
        beads: beads.iter().map(|b| b.to_string()).collect(),
        open_beads: open,
        merge_commit: (result != "refused").then(|| "ccc".to_string()),
        pid: None,
        started_at: at.into(),
        finished_at: at.into(),
    };
    let refuted = |bead: &str| OpenBead {
        bead: bead.into(),
        why: "\"docs/absent.md says it\": the merge did not change docs/absent.md".into(),
        refuted: true,
        contradicted: "\"docs/absent.md says it\": the merge did not change docs/absent.md".into(),
    };
    let five = ["zz-7p85", "zz-epo9", "zz-fsxg", "zz-lqhf", "zz-xeq3"];

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        // The live case: a refusal carrying five beads. Even with a refuted clause on the row
        // it publishes nothing, because nothing landed.
        l.record_landing(&row("r1", "refused", "t1", &five, vec![refuted("zz-7p85")]))
            .map_err(|e| e.to_string())?;
        let refused_publishes_nothing = l.landed_open().map_err(|e| e.to_string())?.is_empty();

        // A real landing that refutes two of them reports both.
        l.record_landing(&row(
            "l1",
            "landed-refuted",
            "t2",
            &five,
            vec![refuted("zz-epo9"), refuted("zz-fsxg")],
        ))
        .map_err(|e| e.to_string())?;
        let mut reported: Vec<String> = l
            .landed_open()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|o| o.bead)
            .collect();
        reported.sort();
        let landed_publishes_all = reported == ["zz-epo9", "zz-fsxg"];

        // A newer refusal of the same branch (main moved under it) leaves both standing.
        l.record_landing(&row("r2", "refused", "t3", &five, vec![]))
            .map_err(|e| e.to_string())?;
        let refusal_does_not_silence = l.landed_open().map_err(|e| e.to_string())?.len() == 2;
        Ok((
            refused_publishes_nothing && refusal_does_not_silence,
            landed_publishes_all,
        ))
    })()
    .unwrap_or_else(blocked);
    let (red, green) = res;
    Probe {
        name: "land: a refused landing publishes no landed beads and silences no refutation; a landed one publishes all of them",
        red_fires: red,
        green_passes: green,
    }
}

/// air-ppf: the `landed-not-closed` sentence asserts a contradiction, so it may name only the
/// clauses the merge contradicts. It used to render the row's whole `why`, which also carries
/// every clause Air could not read, so "nothing Air can look up" appeared under a CONTRADICTS
/// headline and two sound closes (air-03w, air-97z) each cost a round trip on 2026-08-30.
///
/// Red: a bead with one refuted clause and two unreadable ones is reported naming the refuted
/// clause and neither of the others. Green: a bead with only unreadable clauses produces no
/// CONTRADICTS claim at all, and the row still keeps both halves. The first is what makes the
/// second believable: a message that only ever names what it can refute can be read at face
/// value.
fn probe_contradicts_names_only_the_refuted() -> Probe {
    use crate::cmd::acceptance::{Evidence, judge_clauses};
    use crate::cmd::status::{Snapshot, Thresholds, attention, kinds};
    use air_ledger::landings::{Landing, OpenBead};

    let changed = vec!["docs/rules/roles.md".to_string()];
    let tree = vec![
        "docs/rules/roles.md".to_string(),
        "docs/rules/writing.md".to_string(),
        "docs/absent.md".to_string(),
    ];
    let ev = Evidence {
        green_at_landed: true,
        changed: &changed,
        tree: &tree,
    };
    let mixed = judge_clauses(
        "zz-1",
        vec![
            "docs/absent.md says it.".into(),
            "The owner rules on the counter-argument.".into(),
            "Docs are updated.".into(),
        ],
        &ev,
    );
    let unreadable_only = judge_clauses("zz-2", vec!["The owner rules on it.".into()], &ev);
    let as_row = |j: &crate::cmd::acceptance::Judged| OpenBead {
        bead: j.bead.clone(),
        why: j.why_open(),
        refuted: j.refuted(),
        contradicted: j.why_contradicted(),
    };

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_landing(&Landing {
            despite_inflight: vec![],
            members: vec![],
            id: new_id(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: "landed-refuted".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 1,
            beads: vec!["zz-1".into(), "zz-2".into()],
            open_beads: vec![as_row(&mixed), as_row(&unreadable_only)],
            merge_commit: Some("ccc".into()),
            pid: None,
            started_at: "t0".into(),
            finished_at: "t1".into(),
        })
        .map_err(|e| e.to_string())?;
        let snap = Snapshot {
            landed_open: l.landed_open().map_err(|e| e.to_string())?,
            ..Default::default()
        };
        let att = attention(&snap, "2026-09-05T00:00:00Z", Thresholds::default());
        let landed: Vec<_> = att
            .iter()
            .filter(|a| a.kind == kinds::LANDED_NOT_CLOSED)
            .collect();
        let names_the_refuted_alone = landed.len() == 1
            && landed.first().is_some_and(|a| {
                a.worker == "zz-1"
                    && a.detail
                        .contains("CONTRADICTS: \"docs/absent.md says it.\"")
                    && !a.detail.contains("nothing Air can look up")
                    && !a.detail.contains("owner rules")
                    && !a.detail.contains("Docs are updated")
            });
        let row_keeps_both = l
            .landings()
            .map_err(|e| e.to_string())?
            .first()
            .and_then(|r| r.open_beads.iter().find(|o| o.bead == "zz-1").cloned())
            .is_some_and(|o| {
                o.why.contains("nothing Air can look up") && o.why.contains("docs/absent.md")
            });
        let unreadable_is_silent = !landed.iter().any(|a| a.worker == "zz-2");
        Ok((
            names_the_refuted_alone,
            unreadable_is_silent && row_keeps_both,
        ))
    })()
    .unwrap_or_else(blocked);
    let (red_fires, green_passes) = res;

    Probe {
        name: "status: landed-not-closed names only the clauses the merge contradicts; a bead Air merely could not read makes no CONTRADICTS claim",
        red_fires,
        green_passes,
    }
}

/// air-dqa: a path Air read out of prose and got wrong must not become a confident false
/// accusation. Three firings of `landed-not-closed`, zero true: the adopter's clause wrote a
/// possessive (`docs/reference/tooling.md`'s), the trim stopped at the `s`, the token matched
/// nothing in a merge that had changed that very file, and Air reported CONTRADICTED.
///
/// Red: that clause, verbatim, against a merge that changed the file, is UNREADABLE with the
/// token named, not refuted. Green: the ai_runner case, a clause naming an existing file the
/// work correctly did not touch (the pin landed in install.rs, not install_and_launch.rs),
/// stays REFUTED — the true fact, for a person to read — and the plainly written possessive
/// clause discharges. The second is the true positive the first must not cost.
fn probe_unresolvable_path_is_unreadable_not_refuted() -> Probe {
    use crate::cmd::acceptance::{Evidence, Verdict, judge};

    let tree = vec![
        "docs/reference/tooling.md".to_string(),
        "crates/cli/tests/install_and_launch.rs".to_string(),
        "crates/cli/src/cmd/install.rs".to_string(),
    ];
    let adopter = Evidence {
        green_at_landed: true,
        changed: &["docs/reference/tooling.md".to_string()],
        tree: &tree,
    };
    let possessive = "Air's own `docs/reference/tooling.md`'s section is updated.";
    let red_fires = matches!(
        judge(possessive, &adopter),
        Verdict::Undecidable { how } if how.contains("cannot resolve") && how.contains("tooling.md`'s")
    );

    let ai_runner = Evidence {
        changed: &["crates/cli/src/cmd/install.rs".to_string()],
        ..adopter
    };
    let untouched_stays_refuted = matches!(
        judge("Pin it in crates/cli/tests/install_and_launch.rs.", &ai_runner),
        Verdict::Unevidenced { how }
            if how == "the merge did not change crates/cli/tests/install_and_launch.rs"
    );
    let plain_discharges = judge("docs/reference/tooling.md is updated.", &adopter).discharged();

    Probe {
        name: "acceptance: a path-like token that is no file at the landed commit is unreadable, not refuted; an existing untouched file still is",
        red_fires,
        green_passes: untouched_stays_refuted && plain_discharges,
    }
}

/// air-09b: a bead is a handle on a branch only while one branch carries it. The adopter,
/// 2026-08-30, twice: a bead carried by a batching lane and by the worker it batched. Named,
/// `air land` took the oldest-waiting branch (the worker's), main moved, and the lane was
/// refused; with the worker's branch blocked, the bead was refused outright.
///
/// Red, both observed cases: two landable carriers is refused naming each with `--worker`; a
/// landable carrier beside a blocked one is refused the same way, with the landable one's
/// command and the blocked one's fix, not silently resolved by state. Green: `--worker` lands
/// that branch with every bead it carries; a bead on ONE blocked branch is still refused with
/// that branch's fix; a bead on one landable branch still lands.
fn probe_land_names_a_branch() -> Probe {
    use crate::cmd::land::resolve;
    use crate::cmd::status::Landing;

    let landing = |worker: &str, bead: &str, minutes: i64, blocked: Option<&str>| Landing {
        bead: bead.into(),
        worker: worker.into(),
        head: format!("{worker}0000"),
        minutes,
        command: match blocked {
            None => format!("air land --worker {worker}"),
            Some(_) => "git merge main && air record verify -- make verify".into(),
        },
        acceptance: Vec::new(),
        blocked: blocked.map(String::from),
    };
    let none: Vec<String> = Vec::new();
    let fd1 = vec!["zz-1".to_string()];
    let fd2 = vec!["zz-2".to_string()];

    // Case 1: alpha did fd-1 and has waited longest; lane batched it and carries fd-2 too.
    let both_ready = vec![
        landing("alpha", "zz-1", 30, None),
        landing("lane", "zz-1", 5, None),
        landing("lane", "zz-2", 5, None),
    ];
    let case1 = matches!(
        resolve(&fd1, &none, &both_ready, &[], &[]),
        Err(m) if m.contains("--worker alpha") && m.contains("--worker lane")
    );
    // Case 2: alpha's branch is behind main now; lane can land.
    let blocked = vec![landing("alpha", "zz-1", 30, Some("does not contain main"))];
    let lane_ready = vec![
        landing("lane", "zz-1", 5, None),
        landing("lane", "zz-2", 5, None),
    ];
    let case2 = matches!(
        resolve(&fd1, &none, &lane_ready, &blocked, &[]),
        Err(m) if m.contains("--worker lane") && m.contains("does not contain main")
            && !m.contains("--worker alpha")
    );

    let selector = matches!(
        resolve(&none, &["lane".to_string()], &lane_ready, &blocked, &[]),
        Ok(v) if v.len() == 2 && v.iter().all(|l| l.worker == "lane")
    );
    let single_blocked = matches!(
        resolve(&fd1, &none, &[], &blocked, &[]),
        Err(m) if m.contains("not landable yet") && m.contains("does not contain main")
    );
    // air-dnr: naming fd-2 selects lane's branch, which carries fd-1 too. This used to assert
    // `v.len() == 1`, which was the defect written down as the expectation.
    let single_ready = matches!(
        resolve(&fd2, &none, &lane_ready, &blocked, &[]),
        Ok(v) if v.len() == 2 && v.iter().all(|l| l.worker == "lane")
    );

    Probe {
        name: "land: a bead on two branches is refused naming each with --worker; --worker lands that branch with every bead it carries; a bead on one blocked branch is still refused with its fix",
        red_fires: case1 && case2,
        green_passes: selector && single_blocked && single_ready,
    }
}

/// air-0kk: a release reopens AND unassigns in one bd process. Reopening alone left the
/// assignee pencilled in, which in bd 1.2.x blocks every other worker's `--claim`: the bead
/// sat in `bd ready` claimable by nobody but the worker that had released it (the adopter
///; air-an9 here after gate's session was gone).
///
/// Red: the release argv clears the assignee in the same process that sets the status.
/// Green: a plain status write still leaves the assignee alone (a close keeps its closer),
/// and the id is in the right place.
fn probe_release_unassigns() -> Probe {
    use air_bd::reopen_argv;

    let argv = reopen_argv("zz-1");
    let has = |a: &str, b: &str| {
        argv.windows(2)
            .any(|w| matches!(w, [x, y] if x == a && y == b))
    };
    let red = argv.first().is_some_and(|c| c == "update")
        && argv.get(1).is_some_and(|id| id == "zz-1")
        && has("-s", "open")
        && has("-a", "");
    let green = argv.len() == 6 && argv.iter().filter(|a| *a == "-a").count() == 1;
    Probe {
        name: "release: reopening a bead clears its assignee in the same bd process, so anyone can claim it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-gsj: `air claim` retries bd exactly once, and only on a timeout. The adopter's w1 retried
/// a claim by hand three times and another worker took the bead in between; the message read
/// as a denial. A fresh process starts at bd's ~2 s floor while any usable timeout is crossed
/// by the same stalls (air-bp0), so the retry has a mechanism behind it and a longer wait does
/// not.
///
/// Red: a timeout followed by an answer is retried once and the answer is returned. Green: a
/// refusal is bd's answer and is not retried; a second timeout is returned after exactly two
/// attempts, never a third.
fn probe_claim_retries_a_timeout_once() -> Probe {
    use crate::cmd::claim::retry_once;
    use air_bd::BdError;
    use std::time::Duration;

    let timeout = || BdError::Timeout(Duration::from_secs(1));
    let refusal = || BdError::Failed {
        code: 1,
        stderr: "no".into(),
    };

    // Timeout, then an answer.
    let mut calls: u32 = 0;
    let mut noted = false;
    let (r, retried) = retry_once(
        || {
            calls = calls.saturating_add(1);
            if calls == 1 { Err(timeout()) } else { Ok(42) }
        },
        || noted = true,
    );
    let red = matches!(r, Ok(42)) && retried && calls == 2 && noted;

    // A refusal: bd answered, so no retry.
    let mut calls: u32 = 0;
    let (r, retried) = retry_once(
        || {
            calls = calls.saturating_add(1);
            Err::<i32, _>(refusal())
        },
        || {},
    );
    let refusal_not_retried = matches!(r, Err(BdError::Failed { .. })) && !retried && calls == 1;
    // Two timeouts: two attempts, then the timeout is reported.
    let mut calls: u32 = 0;
    let (r, retried) = retry_once(
        || {
            calls = calls.saturating_add(1);
            Err::<i32, _>(timeout())
        },
        || {},
    );
    let twice_then_stop = matches!(r, Err(BdError::Timeout(_))) && retried && calls == 2;

    Probe {
        name: "claim: a bd timeout is retried once and a refusal never is; two timeouts stop at two attempts",
        red_fires: red,
        green_passes: refusal_not_retried && twice_then_stop,
    }
}

/// air-bh4: bd not answering about a bead's acceptance REFUSES the landing before anything
/// moves; it does not become an empty clause list. The row for a timed-out landing used to say
/// "the bead states no acceptance criteria ... so Air read nothing to check" about a bead with
/// four criteria Air never read (the adopter), and the wrong-close check read that row as
/// a clean result.
///
/// Red: a bd error yields a refusal naming bd, the beads, and "Nothing was changed", and it
/// does not contain the no-criteria sentence. Green: a bead that genuinely states none still
/// judges as "states no acceptance criteria" — the two are different artefacts (a refusal
/// string versus a landed row's `why`), which is what the bead asks for.
fn probe_acceptance_unread_refuses() -> Probe {
    use crate::cmd::acceptance::{Evidence, judge_clauses};
    use crate::cmd::land::acceptance_read;

    let beads = vec!["zz-0vh3".to_string()];
    let refused = acceptance_read(
        Err("bd show for zz-0vh3: timed out after 10s".into()),
        &beads,
    );
    let red = matches!(&refused, Err(m) if m.starts_with("refused:")
        && m.contains("bd did not answer for zz-0vh3")
        && m.contains("Nothing was changed")
        && !m.contains("no acceptance criteria"));

    let changed: Vec<String> = Vec::new();
    let tree: Vec<String> = Vec::new();
    let ev = Evidence {
        green_at_landed: true,
        changed: &changed,
        tree: &tree,
    };
    let answered = acceptance_read(Ok(vec![Vec::new()]), &beads);
    let none = judge_clauses("zz-0vh3", Vec::new(), &ev);
    let green = matches!(&answered, Ok(c) if c.len() == 1 && c.first().is_some_and(Vec::is_empty))
        && none.why_open().contains("states no acceptance criteria")
        && !none.all_discharged()
        && !none.refuted();

    Probe {
        name: "land: bd not answering about acceptance refuses before the merge; a bead that states none still lands as 'none'",
        red_fires: red,
        green_passes: green,
    }
}

/// air-dnr: `air land <bead>` records every bead the branch's merge range names, not the one
/// typed. The merge is per branch; the argument selects the branch. The adopter, 2026-08-30:
/// `air land ` on a lane carrying five beads recorded one, and four landed with no
/// acceptance check and no wrong-close detection.
///
/// Red: naming ONE bead on a branch carrying five selects all five, once each, and naming
/// two of them still yields the five once. Green: a bead no branch names is still refused,
/// and a bead on a different branch is not swept in.
fn probe_land_by_bead_carries_the_whole_branch() -> Probe {
    use crate::cmd::land::resolve;
    use crate::cmd::status::Landing;

    let landing = |worker: &str, bead: &str| Landing {
        bead: bead.into(),
        worker: worker.into(),
        head: format!("{worker}0000"),
        minutes: 5,
        command: format!("air land --worker {worker}"),
        acceptance: Vec::new(),
        blocked: None,
    };
    let five = ["zz-7p85", "zz-epo9", "zz-fsxg", "zz-lqhf", "zz-xeq3"];
    let mut ready: Vec<Landing> = five.iter().map(|b| landing("lane", b)).collect();
    ready.push(landing("other", "zz-zzz"));
    let none: Vec<String> = Vec::new();
    fn beads_of(v: &[Landing]) -> Vec<&str> {
        let mut b: Vec<&str> = v.iter().map(|l| l.bead.as_str()).collect();
        b.sort_unstable();
        b
    }

    let one = resolve(&["zz-fsxg".to_string()], &none, &ready, &[], &[]);
    let all_five =
        matches!(&one, Ok(v) if beads_of(v) == five && v.iter().all(|l| l.worker == "lane"));
    let two = resolve(
        &["zz-7p85".to_string(), "zz-xeq3".to_string()],
        &none,
        &ready,
        &[],
        &[],
    );
    let once_each = matches!(&two, Ok(v) if beads_of(v) == five);

    let absent = matches!(
        resolve(&["zz-nope".to_string()], &none, &ready, &[], &[]),
        Err(m) if m.contains("no green branch names zz-nope")
    );
    let not_swept = matches!(
        resolve(&["zz-zzz".to_string()], &none, &ready, &[], &[]),
        Ok(v) if v.len() == 1 && v.first().is_some_and(|l| l.worker == "other")
    );

    Probe {
        name: "land: naming one bead lands and records every bead its branch carries, once each; an unnamed bead is still refused and another branch is not swept in",
        red_fires: all_five && once_each,
        green_passes: absent && not_swept,
    }
}

/// air-0y9: a mechanism that fires with no registry row must be reported, not omitted. A
/// registry that silently drops one reads as complete when it is not. Red: an unclaimed
/// command/decision pair is a defect. Green: the same pair, once a row claims it, is counted
/// as that mechanism instead.
fn probe_audit_unregistered_firing() -> Probe {
    use crate::cmd::audit::{gather_from, registered_traces};

    let unclaimed = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"beta","command":"hook.Whatever","decision":"throttled"}"#,
        "\n",
    );
    let red = {
        let a = gather_from(
            &[("2026-08-22".to_string(), unclaimed.to_string())],
            "2026-08-22",
        );
        // Named, with its count, rather than dropped for being an unfamiliar decision word.
        a.unregistered == vec![("hook.Whatever / throttled".to_string(), 1)]
            && crate::cmd::audit::render(&a).contains("defect:")
    };
    // Green: a pair the registry does claim is attributed to its mechanism and is not a
    // defect. `claim / refuse` is the row air-0y9 added.
    let claimed = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"beta","command":"claim","decision":"refuse"}"#,
        "\n",
    );
    let green = {
        let a = gather_from(
            &[("2026-08-22".to_string(), claimed.to_string())],
            "2026-08-22",
        );
        registered_traces().contains("claim / refuse")
            && a.unregistered.is_empty()
            && a.rows
                .iter()
                .any(|r| r.id == "claim-refusal" && r.evaluations == 1)
    };
    Probe {
        name: "audit: a firing with no registry row is a defect; a claimed pair counts as its mechanism",
        red_fires: red,
        green_passes: green,
    }
}

/// air-6g1: a repo installed before a surface change is told about it, and one already
/// current is told nothing. The diff is keyed to recorded ids, not a version string, so it
/// cannot silently report nothing because a number was not bumped.
fn probe_surface_diff() -> Probe {
    use crate::cmd::install::{SURFACE, surface_diff};

    // Red: a repo that knows about nothing sees every change, `air land` among them.
    let stale = surface_diff(&[]);
    let red = !stale.is_empty()
        && stale.iter().any(|c| c.id == "land")
        && stale.iter().any(|c| c.silent_break);
    // Green: a repo recorded at the current surface sees nothing.
    let current: Vec<String> = SURFACE.iter().map(|c| c.id.to_string()).collect();
    let green = surface_diff(&current).is_empty();
    Probe {
        name: "install: an older recorded surface diffs (names `air land`); the current one is empty",
        red_fires: red,
        green_passes: green,
    }
}

/// air-76z: a capture must not point at a bead bd does not have. bd omits an unknown id from
/// `bd show` and still exits 0, so the check is the comparison, not the exit code.
fn probe_triage_bead_exists() -> Probe {
    use crate::cmd::capture::missing_ids;

    let want: Vec<String> = ["zz-1", "zz-nope", "zz-2"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let issue = |id: &str| air_bd::Issue {
        id: id.to_string(),
        ..Default::default()
    };
    let red = missing_ids(&want, &[issue("zz-1"), issue("zz-2")]) == ["zz-nope"];
    let green = missing_ids(&want, &[issue("zz-1"), issue("zz-nope"), issue("zz-2")]).is_empty();
    Probe {
        name: "triage: a bead bd did not return is named; a full answer passes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-869: `air close` is the coordinator's, and it issues ONE bd process however many
/// beads it is given. Red: a worker is refused. Green: the coordinator's ten ids build a
/// single `bd close` argv and release ten claims in one transaction.
fn probe_batch_close() -> Probe {
    use crate::cmd::close::may_close;

    let red = may_close(Some("beta")).is_err();
    let green = (|| -> Result<bool, String> {
        let ids: Vec<String> = (1..=10).map(|i| format!("zz-{i}")).collect();
        let argv = air_bd::close_argv(&ids, "landed", "main");
        let one_process = argv.first().map(String::as_str) == Some("close")
            && ids.iter().all(|i| argv.contains(i));
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        for id in &ids {
            l.record_claim(id, "beta", &[], "t0")
                .map_err(|e| e.to_string())?;
        }
        let released = l
            .release_claims_on(&ids, "landed", "t1")
            .map_err(|e| e.to_string())?;
        Ok(may_close(Some("main")).is_ok() && one_process && released.len() == ids.len())
    })()
    .unwrap_or(false);
    Probe {
        name: "close: a worker is refused; ten coordinator closes are one bd argv, one transaction",
        red_fires: red,
        green_passes: green,
    }
}

/// air-3pz: `air land` refuses a worker, a dirty main, a branch that has not merged main, and
/// a recorded green that is not at the branch head; a clean green hand-over passes. Pure over
/// the facts, so the whole refusal set fires without a repo.
fn probe_land_refusals() -> Probe {
    use crate::cmd::land::{Facts, Site, check, may_land};

    fn here() -> Site {
        Site {
            on_main: true,
            main_checkout: true,
        }
    }
    let ok = || Facts {
        worker: "alpha",
        branch_exists: true,
        already_in_main: false,
        contains_main: true,
        branch_head: "abcdef99",
        green_at: Some("abcdef99"),
    };
    let sites = [
        Site {
            on_main: false,
            ..here()
        },
        Site {
            main_checkout: false,
            ..here()
        },
    ];
    let refusals = [
        Facts {
            branch_exists: false,
            ..ok()
        },
        Facts {
            contains_main: false,
            ..ok()
        },
        Facts {
            green_at: Some("00000000"),
            ..ok()
        },
        Facts {
            green_at: None,
            ..ok()
        },
    ];
    let readable = |m: String| m.contains('`') && m.starts_with("refused: ");
    // Every refusal fires, and every one names a command to run.
    let red = may_land(&at("alpha", "alpha")).is_err()
        && sites
            .iter()
            .all(|s| check(s, &ok()).err().is_some_and(readable))
        && refusals
            .iter()
            .all(|f| check(&here(), f).err().is_some_and(readable));
    let green = may_land(&at("main", "main")).is_ok()
        && check(&here(), &ok()) == Ok(true)
        && check(
            &here(),
            &Facts {
                already_in_main: true,
                ..ok()
            },
        ) == Ok(false);
    Probe {
        name: "land: worker, dirty main, stale branch and a green off the head are all refused with a fix; a clean green passes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-uir: `idle-without-claim` fires on beads the worker can actually claim.
///
/// It counted bd's raw ready set while `ready_cache::claimable` filters `owner`-labelled beads
/// and is what the Stop nudge uses — two numbers for one thing, and only one of them was work
/// a worker could take. It fired on gate at round end on 2026-08-29 with one ready bead,
/// `air-4t1`, labelled `owner`, which gate had already declined. The claimable count was zero.
///
/// This is a condition whose entire output is "go interrupt a worker", so a false fire is the
/// cheapest possible way to teach a coordinator to ignore conditions.
///
/// Red: an idle claimless worker with one claimable bead is reported. Green: the same worker
/// with a queue of beads it may not claim produces nothing, and `air status` says which count
/// it means rather than leaving a reader to open the bead and find out.
fn probe_idle_without_claim_counts_claimable_only() -> Probe {
    use crate::cmd::status::{Snapshot, Thresholds, attention, render_for_probe};

    let at = |ready: usize, claimable: usize| Snapshot {
        at: "2026-08-29T12:30:00Z".into(),
        workers: vec![crate::cmd::status::WorkerView {
            worker: "gate".into(),
            role: "worker".into(),
            session: Some(crate::cmd::status::Session {
                session_id: "s".into(),
                state: "idle".into(),
                changed_at: "2026-08-29T12:00:00Z".into(),
                pid_alive: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ready_depth: Some(ready),
        claimable_depth: Some(claimable),
        ..Default::default()
    };
    let fires = |s: &Snapshot| {
        attention(s, "2026-08-29T12:30:00Z", Thresholds::default())
            .iter()
            .any(|a| a.kind == "idle-without-claim")
    };

    let red = fires(&at(1, 1));
    // The round-end state: a non-empty queue with nothing in it for this worker.
    let green = !fires(&at(1, 0))
        && !fires(&at(5, 0))
        // ...and the count is not silently reinterpreted: the line names both.
        && render_for_probe(&at(3, 1)).contains("ready: 3 (1 claimable; 2 owner-labelled")
        // When they agree there is nothing to disambiguate and the line stays short.
        && render_for_probe(&at(3, 3)).contains("ready: 3\n");
    Probe {
        name: "attention: idle-without-claim counts beads the worker may claim, not bd's raw ready set",
        red_fires: red,
        green_passes: green,
    }
}

/// air-mun: Air runs exactly one `git merge`, and it is `--ff-only`, so no Air command can
/// observe a merge conflict.
///
/// air-mun asked for the conflicted paths of a landing to be recorded, so `air audit` could
/// answer "conflicts in warned files" with a number and peer-warning could be honestly kept or
/// deleted. air-odv landed the same day and removed the merge that would have produced them:
/// `air land` builds the landing commit with `commit-tree` and fast-forwards onto it. Measured
/// on git 2.51: `--ff-only` against a divergent branch aborts with **zero** conflicted paths
/// and a clean working tree, and `branch_check` refuses such a branch before any git write.
///
/// This probe is the guard on that claim. It is a source check rather than a behaviour check on
/// purpose: the assertion is about what Air *can* run, and a behaviour test can only sample the
/// paths it happens to take. If someone reintroduces a three-way merge, the claim in
/// `audit.rs` — that nothing records conflicts because nothing observes them — silently becomes
/// wrong, and this is what says so.
///
/// Red: a `git merge` without `--ff-only` anywhere in the crate is caught. Green: the one merge
/// that exists is the fast-forward, and the audit's explanation names the real site.
fn probe_air_runs_no_conflicting_merge() -> Probe {
    let land = include_str!("land.rs");
    let audit = include_str!("audit.rs");
    // Every `git` argv Air builds names its subcommand as a bare "merge" element.
    let merges: Vec<&str> = land
        .lines()
        .filter(|l| l.contains("\"merge\"") && !l.trim_start().starts_with("//"))
        .collect();
    let red = merges.len() == 1 && merges.first().is_some_and(|l| l.contains("\"--ff-only\""));
    let green =
        // The audit no longer points at `air land` as the place a conflict is seen...
        !audit.contains("`air land` is where one is observed, so recording")
        // ...and says where they actually happen instead.
        && audit.contains("the workers' own")
        && audit.contains("`git merge main`");
    Probe {
        name: "land: Air runs exactly one git merge and it is --ff-only, so no Air command can see a conflict",
        red_fires: red,
        green_passes: green,
    }
}

/// air-odv: nothing reaches main without a green at the branch head AND main contained.
///
/// Those two together are what make the landing commit's tree byte-identical to the tree the
/// worker verified, which is the entire reason no verify runs at landing time and no rewind is
/// possible. If either can be satisfied without the other, main can move to a commit nothing
/// has verified — the state the old merge-then-rewind design held for minutes at a time, and
/// which twice outlived a killed land (d10ddab, 35660df).
///
/// Red: a branch that contains main but has no green, one whose green is at an older sha, and
/// one green but behind main are each refused. Green: both together pass, and nothing else does.
fn probe_nothing_unverified_reaches_main() -> Probe {
    use crate::cmd::land::{Facts, Site, branch_check};

    let both = || Facts {
        worker: "alpha",
        branch_exists: true,
        already_in_main: false,
        contains_main: true,
        branch_head: "abcdef99",
        green_at: Some("abcdef99"),
    };
    let refused = |f: Facts<'_>| branch_check(&f).is_err();
    let red = refused(Facts {
        green_at: None,
        ..both()
    }) && refused(Facts {
        green_at: Some("00000000"),
        ..both()
    }) && refused(Facts {
        contains_main: false,
        ..both()
    });
    // Only the conjunction lands, and `check` from a clean site agrees with it — the same
    // predicate `air status` uses, so the two cannot drift (air-y3v).
    let site = Site {
        on_main: true,
        main_checkout: true,
    };
    let green = branch_check(&both()) == Ok(true)
        && crate::cmd::land::check(&site, &both()) == Ok(true)
        // Already in main is the one non-refusal that also does not move main.
        && branch_check(&Facts {
            already_in_main: true,
            ..both()
        }) == Ok(false);
    Probe {
        name: "land: main moves only for a branch that contains main AND is green at its head, which is why no verify runs there",
        red_fires: red,
        green_passes: green,
    }
}

/// air-03w: a branch that goes green with main merged is a condition, pushed once.
///
/// Since air-7o3 the worker closes its own bead with proof and never sets `awaiting_review`,
/// so `review-waiting`'s subject is a state this repo stopped using: nothing told the
/// coordinator a branch was ready, and it learned by polling `air status`. The worker
/// signalling on close is the intent (roles.md, owner 2026-08-29); this is the failsafe.
///
/// Red: a landable branch produces exactly one push, naming the beads and `air land --all`.
/// Green: it does not repeat while it sits, however long — age is not a change (air-s7c) — and
/// a moved head is a real change that pushes again.
fn probe_landable_pushes_once_per_branch() -> Probe {
    use crate::cmd::mcp::{Pushed, select_new};
    use crate::cmd::status::{Landing, Snapshot, Thresholds, attention};

    let snap = |head: &str, beads: &[&str], minutes: i64| Snapshot {
        landable: beads
            .iter()
            .map(|b| Landing {
                bead: (*b).to_string(),
                worker: "alpha".into(),
                head: head.to_string(),
                minutes,
                command: format!("air land {b}"),
                acceptance: Vec::new(),
                blocked: None,
            })
            .collect(),
        ..Default::default()
    };
    let at = |s: &Snapshot| attention(s, "2026-08-29T12:00:00Z", Thresholds::default());

    let first = at(&snap("abcdef1234", &["air-1", "air-2"], 5));
    let landable: Vec<_> = first.iter().filter(|a| a.kind == "landable").collect();
    // One condition for the branch, however many beads it carries: one branch is one merge.
    let red = landable.len() == 1
        && landable.first().is_some_and(|a| {
            a.worker == "alpha"
                && a.detail.contains("air-1 air-2")
                && a.detail.contains("air land --all")
                && a.detail.contains("abcdef12")
        });

    let mut pushed = Pushed::new();
    let pushed_first = select_new(
        &mut pushed,
        &at(&snap("abcdef1234", &["air-1", "air-2"], 5)),
    );
    // Still sitting there an hour later, and a bead count that changed without the head
    // moving: neither is a new fact about whether the branch can land.
    let sitting = select_new(
        &mut pushed,
        &at(&snap("abcdef1234", &["air-1", "air-2", "air-3"], 65)),
    );
    // The head moved: the worker committed and re-verified, so this is a different tree.
    let moved = select_new(&mut pushed, &at(&snap("99999999aa", &["air-1"], 1)));
    let green = pushed_first.iter().filter(|a| a.kind == "landable").count() == 1
        && !sitting.iter().any(|a| a.kind == "landable")
        && moved.iter().filter(|a| a.kind == "landable").count() == 1
        // Nothing landable is silent.
        && !at(&snap("x", &[], 0)).iter().any(|a| a.kind == "landable");
    Probe {
        name: "landable: a branch green with main merged pushes once per head, not while it sits",
        red_fires: red,
        green_passes: green,
    }
}

/// A caller standing in `here` with `--repo` resolving to `pointed`.
fn at<'a>(here: &'a str, pointed: &'a str) -> crate::cmd::land::Caller<'a> {
    crate::cmd::land::Caller {
        where_i_am: Some(here),
        where_i_pointed: pointed,
    }
}

/// air-29a: `air land`'s role comes from where the process is, not from `--repo`.
///
/// The incident: worker beta ran `cargo run -q -p air -- --repo <main> land --all` from its
/// worktree on 2026-08-22 to check its own fix, and it LANDED — merging `worktree-beta` into
/// main at d10ddab. Two guards were supposed to stop it and neither did. `Bash(air land *)`
/// matches command TEXT, so `cargo run`, `./target/debug/air`, and an absolute path all miss
/// it. And `may_land` was fed `worker_name_for(repo)`, where `repo` is `--repo` — an argument
/// the caller supplies, so pointing it at the main checkout made the caller `main`.
///
/// A parser that guards counts as absent until proven present (`anti-brittleness`). Neither of
/// these was present. This probe is what proves the replacement fires.
///
/// Red: a worker is refused standing in its own worktree, refused while pointing `--repo` at
/// the main checkout, and refused when Air cannot tell where it is. The `--repo` case names the
/// bypass. Green: the coordinator standing in the main checkout passes.
///
/// Note what the probe does NOT vary: how the command was spelled. That is the point — argv
/// never reaches this decision, so there is no spelling to enumerate.
fn probe_land_role_is_where_you_are() -> Probe {
    use crate::cmd::land::{Caller, may_land};

    let nowhere = Caller {
        where_i_am: None,
        where_i_pointed: "main",
    };
    let bypass = may_land(&at("alpha", "main"));
    let red = may_land(&at("alpha", "alpha")).is_err()
        // The incident's own invocation: in a worktree, --repo at the main checkout.
        && bypass.as_ref().err().is_some_and(|m| m.contains("air-29a"))
        && bypass
            .as_ref()
            .err()
            .is_some_and(|m| m.contains("cargo run -p air -- land"))
        // Fails closed: an unknown location is not a coordinator.
        && may_land(&nowhere).is_err();
    // The coordinator's ordinary run, and only from the main checkout.
    let green = may_land(&at("main", "main")).is_ok()
        // Standing in main while --repo names a worktree is still the coordinator: the role
        // is where you are, in both directions.
        && may_land(&at("main", "alpha")).is_ok();
    Probe {
        name: "land: the role is where the process is, so --repo at the main checkout does not make a worker the coordinator",
        red_fires: red,
        green_passes: green,
    }
}

/// air-0lk, corrected by air-3oq: a session may ACT only on its own project, and may TALK to
/// any of them. With `AIR_PROJECT=air`, a PreToolUse call for `tmux kill-session -t other-worker1`
/// denies with a refusal that names the fence, the project and what is still allowed; the same
/// call for `air-alpha` passes. `SendMessage` to another project's coordinator passes, which is
/// the half air-0lk got wrong: denying it broke the cross-project channel silently, in the
/// verification path.
/// air-7ah: which project a session belongs to must come from what it is told, not from what
/// happens to be in the ambient environment. `project_for` read `AIR_PROJECT` directly, so the
/// hook test asserting a scratch repo's prefix quietly got the ambient value of whatever
/// session ran it — green everywhere except inside a launched session, which is the only place
/// `air land` runs. Landing was blocked for every branch and the test was hiding it.
///
/// Red: with a project supplied, the supplied value wins over the checkout's beads prefix —
/// the case that was never exercised. Green: with none supplied, the prefix is used, and a
/// blank is not a value. Neither arm reads the environment, so this cannot regress the way it
/// did.
fn probe_project_is_taken_from_what_it_is_told() -> Probe {
    use crate::cmd::hook::project_from;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(dir.join(".beads")).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(".beads/config.yaml"), "issue-prefix: \"zz\"\n")
            .map_err(|e| e.to_string())?;
        let red = project_from(Some("air"), &dir) == "air";
        let green = project_from(None, &dir) == "zz"
            && project_from(Some("   "), &dir) == "zz"
            && project_from(Some(" air "), &dir) == "air";
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "project: the session's project comes from what it is told, not from ambient AIR_PROJECT",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-xbl (the adopter, 2026-08-30/31): two symptoms of one root. With no claim and no bead
/// named, the digest check built an empty bead list, matched nothing, and refused every
/// hand-over from a claimless lane; and with a claim held, the refusal printed a literal
/// `<bead>` because the held id was computed for the lookup and discarded before the message.
/// `air handover`'s printed fix is the one line a worker copies verbatim.
///
/// Red: with one claim held and no bead named, the refusal names that id in both the detail
/// and the fix, the same id `air status` prints under `claims:` (the back-to-back check
/// The adopter's w3 asked for), and never a placeholder. Green: a worker with no claim and no
/// bead named has no digest check at all (nothing to declare), while a worker that holds a
/// claim or names a bead still has one, so the fix is not a hole.
///
/// The mutation that made it red, seen: `beads_to_name` returning `vec![]` for the unnamed
/// case, which is the id thrown away again.
fn probe_handover_names_the_held_bead_and_skips_with_none() -> Probe {
    use crate::cmd::handover::digest_beads;

    let held = vec!["zz-251z".to_string()];
    let mut f = base_facts();
    f.held_beads = held.clone();
    f.digest_present = Some(false);
    f.digest_dir = Some("docs/log.d".into());
    let v = handover_verdict(&f);
    let names_it = v.missing.iter().any(|m| {
        m.check == "digest-present"
            && m.detail.contains("bead: zz-251z")
            && m.fix.contains("bead: zz-251z")
    });
    // What `air status` prints under `claims:` is the ledger's open claims for the worker,
    // which is exactly `held_beads`; the message names the same id in the same state.
    let same_as_status = held.iter().all(|b| v.message.contains(b.as_str()));
    let red_fires = names_it && same_as_status && !v.message.contains("<bead>");

    let skipped_without = digest_beads(None, &[], &[]).is_none();
    let kept_with_claim = digest_beads(None, &held, &[]) == Some(held.clone());
    let kept_with_name = digest_beads(Some("zz-9"), &[], &[]) == Some(vec!["zz-9".to_string()]);
    let named_wins = digest_beads(Some("zz-9"), &held, &[]) == Some(vec!["zz-9".to_string()]);
    Probe {
        name: "handover: a refusal names the bead the worker holds, never a placeholder; no claim and no bead means no digest check",
        red_fires,
        green_passes: skipped_without && kept_with_claim && kept_with_name && named_wins,
    }
}

/// air-60x (the adopter, 2026-08-31): `air land` says a branch is landable when it
/// contains main and carries a recorded green at its head, and attributes it by `Bead:`
/// trailers; `air handover` additionally demanded an open claim held by the asking worker.
/// So Air would land a branch it refused to let its author hand over, and a branch that
/// superseded another worker's closed bead had no route. Supersession happened twice in one
/// evening there.
///
/// Red: a branch carrying a bead by trailer with no claim on it, a digest declaring it, and
/// a green at a head containing main is handable, and the digest check looks for that bead.
/// Green: the same branch with neither digest nor green is still refused, and the refusal
/// offers the trailer, never `air claim` on a bead that may be closed.
///
/// The mutation that made it red, seen: `handable` ignoring `carried`, which is the gate
/// consulting claims alone again.
fn probe_a_superseding_branch_hands_over_by_its_trailer() -> Probe {
    use crate::cmd::handover::{digest_beads, handable};

    let carried = vec!["zz-x".to_string()];
    let mut ok = base_facts();
    ok.bead = Some("zz-x".into());
    ok.carried_beads = carried.clone();
    ok.bead_claimed_or_carried = handable(Some("zz-x"), false, &carried);
    ok.digest_present = Some(true);
    ok.digest_dir = Some("docs/log.d".into());
    let red_fires =
        handover_verdict(&ok).pass && digest_beads(None, &[], &carried) == Some(carried.clone());

    let mut bad = base_facts();
    bad.bead = Some("zz-x".into());
    bad.carried_beads = vec![];
    bad.bead_claimed_or_carried = handable(Some("zz-x"), false, &[]);
    bad.green_at_head = false;
    bad.digest_present = Some(false);
    bad.digest_dir = Some("docs/log.d".into());
    let v = handover_verdict(&bad);
    let green_passes = !v.pass
        && v.missing.iter().any(|m| m.check == "claim")
        && v.missing.iter().any(|m| m.check == "verify-green-at-head")
        && v.missing.iter().any(|m| m.check == "digest-present")
        && !v.message.contains("air claim")
        && v.message.contains("Bead: zz-x");
    Probe {
        name: "handover: a superseding branch hands over by its `Bead:` trailer; with neither digest nor green it is still refused, and never told to claim",
        red_fires,
        green_passes,
    }
}

/// air-f10 (the adopter's w2, 2026-08-31): `ready: 11 (2 claimable; 9 owner-labelled ...)` when
/// the true claimable count was zero, both "claimable" beads being epics. One predicate,
/// `ready_cache::claimable`, fed the status line, the Stop nudge's offer and
/// `idle-without-claim`, and consulted the label alone; `air claim` had no epic check, so a
/// worker offered a container could pencil an assignee onto it. The failure is the
/// reassuring direction: both numbers are plausible and nothing looks broken.
///
/// Red: a ready set of only epics and owner-labelled beads reports ZERO claimable, names the
/// epics apart on the line, and offers nothing to a nudge. Green: a plain task is still
/// claimable, an owner-labelled epic is the owner's, and the three lists are exactly bd's
/// set, so a matching total is a matching set.
///
/// The mutation that made it red, seen: `split` filing every unlabelled bead as claimable
/// (the epic branch removed), which is the old predicate.
fn probe_ready_split_names_epics_apart() -> Probe {
    use crate::cmd::ready_cache::{claimable, split};
    use crate::cmd::status::{Snapshot, render_for_probe};
    use air_hooks::stop_nudge;

    let issue = |id: &str, labels: &[&str], kind: &str| air_bd::Issue {
        id: id.to_string(),
        labels: labels.iter().map(|s| s.to_string()).collect(),
        issue_type: kind.to_string(),
        ..Default::default()
    };
    let mut ready = vec![issue("zz-7vw", &[], "epic"), issue("zz-w00", &[], "epic")];
    for n in 0..9 {
        ready.push(issue(&format!("zz-o{n}"), &["owner"], "task"));
    }
    let s = split(&ready);
    let line = render_for_probe(&Snapshot {
        ready_depth: Some(ready.len()),
        claimable_depth: Some(s.claimable.len()),
        epic_depth: Some(s.epics.len()),
        ..Default::default()
    });
    let red_fires = s.claimable.is_empty()
        && s.epics == ["zz-7vw", "zz-w00"]
        && s.owner.len() == 9
        && line.contains(
            "ready: 11 (0 claimable; 2 epic(s) to decompose, not claimable; 9 owner-labelled",
        )
        && stop_nudge("worker", false, &claimable(&ready), false).is_none();

    ready.push(issue("zz-task", &[], "task"));
    ready.push(issue("zz-oe", &["owner"], "epic"));
    let s = split(&ready);
    let mut all: Vec<String> = s
        .claimable
        .iter()
        .chain(&s.epics)
        .chain(&s.owner)
        .cloned()
        .collect();
    all.sort();
    let mut bds: Vec<String> = ready.iter().map(|i| i.id.clone()).collect();
    bds.sort();
    let green_passes = s.claimable == ["zz-task"]
        && s.epics == ["zz-7vw", "zz-w00"]
        && s.owner.contains(&"zz-oe".to_string())
        && all == bds;
    Probe {
        name: "status: the ready line names epics apart from claimable work; a set of only epics and owner beads is zero claimable, and the split is exactly bd's set",
        red_fires,
        green_passes,
    }
}

/// air-v7o (the adopter, 2026-08-30): `uncommitted` and `journaled` printed identically and
/// neither said when. w1 nearly released a bead over an `uncommitted` that was nine minutes of
/// regenerated fixtures during w3's full verify and had evaporated by the time they checked;
/// w3 had earlier nearly stood down over a `journaled` for work landed hours before. The tag
/// answered "dirty right now?" to a reader who needed "is another agent working here?".
///
/// Red: a file made dirty by a verify and never edited by a tool reads as unjournaled dirt
/// with the verify named, and once clean it is no holding at all (nothing journaled, nothing
/// dirty: no tag). Green: a genuine concurrent edit still reads as one, with its age, so the
/// fix is not a silence; and a remembered edit says how old it is and that the tree is clean.
///
/// The mutation that made it red, seen: `tags` printing `uncommitted` for both the journaled
/// and the unjournaled case, which is the old output.
fn probe_holdings_tags_name_their_tense() -> Probe {
    use crate::cmd::holdings::{Holding, tags};

    let now = "2026-08-30T21:15:00Z";
    let dirt = Holding {
        worker: "w3".into(),
        uncommitted: true,
        verify_in_flight: true,
        ..Default::default()
    };
    let cleaned = Holding {
        worker: "w3".into(),
        ..Default::default()
    };
    let red_fires = tags(&dirt, now).contains("no edit journaled")
        && tags(&dirt, now).contains("verify in flight")
        && !tags(&dirt, now).contains("edited")
        && tags(&cleaned, now).is_empty();

    let edit = Holding {
        worker: "w1".into(),
        uncommitted: true,
        journaled: true,
        last_edit: Some("2026-08-30T21:12:00Z".into()),
        ..Default::default()
    };
    let remembered = Holding {
        worker: "w1".into(),
        journaled: true,
        last_edit: Some("2026-08-30T15:00:00Z".into()),
        ..Default::default()
    };
    let green_passes = tags(&edit, now) == "uncommitted now, edited 3 min ago"
        && tags(&remembered, now) == "journaled 6 h ago, clean now";
    Probe {
        name: "holdings: every tag names its tense; build dirt is not an edit, a cleaned file is no holding, a live edit still is",
        red_fires,
        green_passes,
    }
}

/// Check 5 (ruling D): digest configured but absent → missing `digest-present`; not
/// configured → not applicable.
fn probe_gate_digest() -> Probe {
    let mut red = base_facts();
    red.digest_present = Some(false);
    red.digest_dir = Some("docs/log.d".into());
    let mut green = base_facts();
    green.digest_present = None;
    Probe {
        name: "gate: digest required when the repo configures digest_dir",
        red_fires: handover_verdict(&red)
            .missing
            .iter()
            .any(|m| m.check == "digest-present"),
        green_passes: handover_verdict(&green).pass,
    }
}

/// air-6di: since air-srv `.air/ledger.db` holds the text of every agent-to-agent message,
/// and `air install` only advised that `.air/` be ignored; a stranger's first `git add -A` is
/// the recorded shape of the failure. Red: a repo where `git check-ignore -q .air` fails is
/// refused by `install --write` with the fix on the line. Green: the same repo with the line
/// added is written. The real binary against a real repo, PATH pointed at this executable so
/// the "is the air on PATH" check passes.
fn probe_install_refuses_unignored_air() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["commit", "-q", "--allow-empty", "-m", "a"][..],
        ] {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // This executable first, so `which air` is this binary; the rest of PATH after it, so
        // `git` (which the ignore check asks) is still reachable.
        let bin_dir = format!(
            "{}:{}",
            exe.parent()
                .ok_or_else(|| "no parent".to_string())?
                .display(),
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
        );
        let install = || -> Result<(i32, String), String> {
            let out = air_command(&exe, &dir)
                .arg("--repo")
                .arg(&dir)
                .args(["install", "--write"])
                .env("PATH", &bin_dir)
                .env("AIR_BD_BIN", "/nonexistent/bd")
                .output()
                .map_err(|e| e.to_string())?;
            Ok((
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).to_string(),
            ))
        };
        let (code, err) = install()?;
        let red = code == 2 && err.contains("echo '.air/' >> .gitignore");
        std::fs::write(dir.join(".gitignore"), ".air/\n").map_err(|e| e.to_string())?;
        let (code, err) = install()?;
        let green = code == 0 && !err.contains("check-ignore");
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "install: --write refuses while .air/ is not ignored, naming the fix; ignored, it writes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-yol: the digest refusal's fix is right and, followed, produced the next refusal: the
/// digest commit moves HEAD off the recorded green (the adopter, 2026-08-31, two
/// workers). Red: with a green at HEAD, the refusal says so and names the order that works
/// (commit, merge main, record verify LAST). Green: with no green at HEAD there is nothing to
/// invalidate and the note is absent, so it is not the unconditional noise air-5wq refused.
fn probe_digest_refusal_names_the_order_only_with_a_green() -> Probe {
    let mut with = base_facts();
    with.digest_present = Some(false);
    with.digest_dir = Some("docs/log.d".into());
    with.green_at_head = true;
    let mut without = with.clone();
    without.green_at_head = false;
    let fix_of = |f: &air_hooks::GateFacts| -> String {
        handover_verdict(f)
            .missing
            .iter()
            .find(|m| m.check == "digest-present")
            .map(|m| m.fix.clone())
            .unwrap_or_default()
    };
    let w = fix_of(&with);
    let wo = fix_of(&without);
    Probe {
        name: "gate: the digest refusal says the digest commit moves HEAD off the green and names the order, only when a green is at HEAD",
        red_fires: w.contains("moves HEAD off")
            && w.contains("air record verify -- make verify")
            && w.contains("LAST"),
        green_passes: !wo.is_empty() && !wo.contains("moves HEAD off"),
    }
}

/// air-tdc: `air worker --task` from a socket stdin (the coordinator's Bash tool) must not
/// exec `claude --tmux` (tcgetattr fails there). Red: the socket case is routed away from
/// exec. Green: a detached tmux session is actually created (pure check only when tmux is
/// absent; the probe name says so).
fn probe_launch_no_tty() -> Probe {
    use crate::cmd::launch::{Launch, launch_mode, tmux_session_argv};
    let red =
        launch_mode(false, true) == Launch::Detached && launch_mode(true, true) == Launch::Exec;
    if Command::new("tmux").arg("-V").output().is_err() {
        return Probe {
            name: "launch: socket stdin never execs claude --tmux (tmux absent: pure check only)",
            red_fires: red,
            green_passes: launch_mode(false, false) == Launch::Exec,
        };
    }
    let socket = format!("air-selftest-{}", std::process::id());
    let name = "air-selftest";
    let argv = tmux_session_argv(
        name,
        Path::new("/"),
        Some(&socket),
        &[],
        true,
        "sh",
        &["-c".to_string(), "sleep 30".to_string()],
    );
    let started = Command::new("tmux")
        .args(&argv)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let exists = started
        && Command::new("tmux")
            .args(["-L", &socket, "has-session", "-t", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    let _ = Command::new("tmux")
        .args(["-L", &socket, "kill-server"])
        .output();
    Probe {
        name: "launch: socket stdin starts a detached tmux session instead of exec",
        red_fires: red,
        green_passes: exists,
    }
}

/// Leases: a healthy holder denies a second taker; a dead holder is broken and taken.
fn probe_lease_take() -> Probe {
    use air_ledger::leases::{Holder, Lease, Take};
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let a = Holder {
            worker: "a",
            session_id: None,
            pid: Some(1),
            pid_started: None,
        };
        let b = Holder {
            worker: "b",
            session_id: None,
            pid: Some(2),
            pid_started: None,
        };
        let healthy = |_: &Lease| None;
        l.lease_take("runtime", &a, "api", "t0", healthy)
            .map_err(|e| e.to_string())?;
        let denied = matches!(
            l.lease_take("runtime", &b, "sim", "t1", healthy)
                .map_err(|e| e.to_string())?,
            Take::Held(_)
        );
        let dead = |_: &Lease| Some("dead".to_string());
        let taken = matches!(
            l.lease_take("runtime", &b, "sim", "t2", dead)
                .map_err(|e| e.to_string())?,
            Take::TakenAfter(_)
        );
        Ok((denied, taken))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "lease: healthy holder denies; dead holder is broken and taken",
        red_fires: red,
        green_passes: green,
    }
}

/// air-i59: with `AIR_ENFORCE=1` the PreToolUse gate denies `bd update x -s awaiting_review`
/// when no green is recorded at HEAD, and the reason names the fixing command; once a green
/// verify run is recorded at HEAD (main merged) the same command is allowed.
fn probe_enforced_gate() -> Probe {
    use air_hooks::HookOutcome;
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let head = g(&["rev-parse", "HEAD"])?;
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("zz-1", "probe", &[], "t0")
            .map_err(|e| e.to_string())?;
        let cmd = "bd update zz-1 -s awaiting_review";
        let red = handover_gate(&l, "probe", &dir, cmd, true)?;
        let red_fires = matches!(&red.outcome, HookOutcome::Block { reason }
            if reason.contains("air record verify -- make verify"));
        l.record_verify(&VerifyRun {
            id: new_id(),
            worker: "probe".into(),
            sha: head,
            kind: Kind::Verify,
            exit_code: 0,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: "t1".into(),
            finished_at: "t1".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: None,
            members: vec![],
        })
        .map_err(|e| e.to_string())?;
        let green = handover_gate(&l, "probe", &dir, cmd, true)?;
        let green_passes = matches!(green.outcome, HookOutcome::Allow { context: None });
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red_fires, green_passes))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "gate: AIR_ENFORCE=1 denies bd update -s awaiting_review without green at HEAD (names the fix); allows with green",
        red_fires: red,
        green_passes: green,
    }
}

/// Check 4: a hand-over names a bead the worker neither holds nor carries → missing `claim`.
/// Held or carried by trailer (air-60x) passes; the pure decision is `handover::handable`.
fn probe_gate_claim() -> Probe {
    use crate::cmd::handover::handable;

    let mut red = base_facts();
    red.bead = Some("zz-1".into());
    red.bead_claimed_or_carried = false;
    let mut green = base_facts();
    green.bead = Some("zz-1".into());
    green.bead_claimed_or_carried = true;
    let carried = vec!["zz-1".to_string()];
    Probe {
        name: "gate: the named bead must be claimed by the worker or carried by a `Bead:` trailer in main..HEAD",
        red_fires: handover_verdict(&red)
            .missing
            .iter()
            .any(|m| m.check == "claim")
            && !handable(Some("zz-1"), false, &[]),
        green_passes: handover_verdict(&green).pass
            && handable(Some("zz-1"), true, &[])
            && handable(Some("zz-1"), false, &carried)
            && handable(None, false, &[]),
    }
}

/// The ledger half of `air claim`: a second worker finds the open claim; the same worker
/// re-claiming after release gets a fresh row.
fn probe_claim_cas() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("zz-1", "w1", &[], "t0")
            .map_err(|e| e.to_string())?;
        let held_by_other = l
            .open_claim("zz-1")
            .map_err(|e| e.to_string())?
            .is_some_and(|c| c.worker != "w2");
        l.release_claim("zz-1", "w1", "abandoned", "t1")
            .map_err(|e| e.to_string())?;
        let free = l.open_claim("zz-1").map_err(|e| e.to_string())?.is_none();
        Ok((held_by_other, free))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "claim: ledger sees another worker's open claim; release frees it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-jc0: a timestamp `minutes` before `now`, for a fixture whose age is DERIVED from the
/// threshold that owns it instead of copied next to it.
///
/// `None` when the arithmetic does not land on a real instant. Every call site treats that as
/// a hard failure and goes red: a probe that cannot find its number must say so, never fall
/// back to a guess that happens to pass.
fn minutes_before(now: &str, minutes: i64) -> Option<String> {
    let t: jiff::Timestamp = now.parse().ok()?;
    let span = jiff::Span::new().try_minutes(minutes).ok()?;
    Some(t.checked_sub(span).ok()?.to_string())
}

/// Attention conditions fire on a session idle past the line with a claim held, and stay quiet
/// under it.
///
/// air-jc0: the two ages are read out of the threshold rather than written beside it.
/// The adopter's is the reason — their log-cap probe asserted 45 against a cap the owner
/// had raised to 100, so the probe failed ON THE RULE BEING CORRECT, and the fix was not a
/// bigger number but reading the cap from the script that owns it. Their two controls, both run
/// against this probe (digest 2026-08-29-diligence-air-jc0): with the arm neutralised it goes
/// red; with the threshold moved it stays green and renames itself. Copying the numbers passed
/// the first control and failed the second.
///
/// Repointed from `stuck` to `idle-with-claim` on 2026-08-29 (air-dqw), when `stuck` was
/// deleted. The subject of the probe is unchanged and is not `stuck`: it is that a threshold is
/// read from the rule that owns it. `idle_with_claim_min` is the natural stand-in because
/// `idle` is a state the `sessions` table actually holds, which `stuck` never was.
fn probe_attention() -> Probe {
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention};
    use air_ledger::claims::Claim;
    let mk = |changed: &str| Snapshot {
        workers: vec![WorkerView {
            worker: "w".into(),
            role: "worker".into(),
            claims: vec![Claim {
                bead: "air-1".into(),
                worker: "w".into(),
                claimed_at: "2026-08-20T10:00:00Z".into(),
                declared_files: Vec::new(),
                first_handover_at: None,
                last_handover_at: None,
                handover_attempts: 0,
                released_at: None,
                release_reason: None,
            }],
            session: Some(Session {
                session_id: "s".into(),
                state: "idle".into(),
                detail: None,
                changed_at: changed.into(),
                pid: None,
                pid_alive: None,
                project: String::new(),
                model: String::new(),
                enforce: None,
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let now = "2026-08-20T12:00:00Z";
    let t = Thresholds::default();
    // One minute past the line and one minute short of it, wherever the line currently is.
    let (Some(over), Some(under)) = (
        t.idle_with_claim_min
            .checked_add(1)
            .and_then(|m| minutes_before(now, m)),
        t.idle_with_claim_min
            .checked_sub(1)
            .and_then(|m| minutes_before(now, m)),
    ) else {
        return Probe {
            name: "attention: idle-with-claim threshold could not be read",
            red_fires: false,
            green_passes: false,
        };
    };
    let red = attention(&mk(&over), now, Thresholds::default());
    let green = attention(&mk(&under), now, Thresholds::default());
    Probe {
        name: IDLE_CLAIM_NAME.get_or_init(|| {
            format!(
                "attention: an idle session holding a claim fires at idle_with_claim_min={} min and is quiet under it",
                t.idle_with_claim_min
            )
        }),
        red_fires: red.iter().any(|a| a.kind == "idle-with-claim"),
        green_passes: green.is_empty(),
    }
}

/// The probe name carries the threshold it read, so a changed rule RENAMES the probe instead of
/// breaking it — the adopter's second control made visible in the output.
static IDLE_CLAIM_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static STANDSTILL_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// air-e7q, the standstill: an idle worker with no claim while beads are ready. Red: the
/// condition fires on those facts (the old `attention` was silent on them). Green: the same
/// fleet with the worker fresh and nothing ready is quiet.
///
/// The other half of this probe was `review-waiting`, deleted by air-okc: it reported a bead
/// sitting in `awaiting_review`, and the repo stopped using that state on 2026-08-22 (air-7o3,
/// close-with-proof). It last fired 2026-08-22T19:31 and never again in five recorded days.
fn probe_standstill() -> Probe {
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention};
    let mk = |changed: &str, ready: usize| Snapshot {
        workers: vec![WorkerView {
            worker: "w".into(),
            role: "worker".into(),
            head: Some("abc".into()),
            green_at_head: Some(true),
            session: Some(Session {
                session_id: "s".into(),
                state: "idle".into(),
                detail: None,
                changed_at: changed.into(),
                pid: None,
                pid_alive: None,
                project: String::new(),
                model: String::new(),
                enforce: None,
            }),
            ..Default::default()
        }],
        // air-uir: this probe is about the threshold/liveness, so the two counts agree
        // here; the counting rule itself is probe_idle_without_claim_counts_claimable_only.
        ready_depth: Some(ready),
        claimable_depth: Some(ready),
        ..Default::default()
    };
    let now = "2026-08-20T12:00:00Z";
    let t = Thresholds::default();
    // air-jc0: both ages come out of `idle_noclaim_min`, the threshold that decides this
    // condition, so moving the rule moves the fixture with it.
    let (Some(over), Some(under)) = (
        t.idle_noclaim_min
            .checked_add(1)
            .and_then(|m| minutes_before(now, m)),
        t.idle_noclaim_min
            .checked_sub(1)
            .and_then(|m| minutes_before(now, m)),
    ) else {
        return Probe {
            name: "attention: idle-without-claim threshold could not be read",
            red_fires: false,
            green_passes: false,
        };
    };
    let red = attention(&mk(&over, 5), now, Thresholds::default());
    let green = attention(&mk(&under, 0), now, Thresholds::default());
    Probe {
        name: STANDSTILL_NAME.get_or_init(|| {
            format!(
                "attention: idle-without-claim fires at idle_noclaim_min={} min; a fresh worker with nothing ready is quiet",
                t.idle_noclaim_min
            )
        }),
        red_fires: red.iter().any(|a| a.kind == "idle-without-claim"),
        green_passes: green.is_empty(),
    }
}

/// air-d10: `idle-without-claim` says "prompt them", so it needs somebody to prompt. Red: a
/// live idle worker past the threshold with beads ready still fires. Green: the same row with
/// the session's process gone is silent — the shape of the two longest-lived rows in this
/// repo's ledger, open 4 885 minutes each.
///
/// The mutation that made it red: dropping `sess.pid_alive != Some(false)` from the arm in
/// `status::attention` puts the dead-session case back and this probe's green half fails.
fn probe_idle_without_claim_needs_a_live_session() -> Probe {
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention};
    let now = "2026-08-20T12:00:00Z";
    let t = Thresholds::default();
    // air-jc0: the age is derived from `idle_noclaim_min`, not written beside it. As filed this
    // probe held 30 min against a threshold of 5 — two copies with one owner, so raising the
    // threshold past 30 would have taken the red side silent while the rule stayed correct.
    let Some(over) = t
        .idle_noclaim_min
        .checked_add(1)
        .and_then(|m| minutes_before(now, m))
    else {
        return Probe {
            name: "attention: idle-without-claim threshold could not be read",
            red_fires: false,
            green_passes: false,
        };
    };
    let mk = |alive: Option<bool>| Snapshot {
        workers: vec![WorkerView {
            worker: "w".into(),
            role: "worker".into(),
            session: Some(Session {
                session_id: "s".into(),
                state: "idle".into(),
                detail: None,
                changed_at: over.clone(),
                pid: Some(1),
                pid_alive: alive,
                project: String::new(),
                model: String::new(),
                enforce: None,
            }),
            ..Default::default()
        }],
        // air-uir: this probe is about the threshold/liveness, so the two counts agree
        // here; the counting rule itself is probe_idle_without_claim_counts_claimable_only.
        ready_depth: Some(2),
        claimable_depth: Some(2),
        ..Default::default()
    };
    let red = attention(&mk(Some(true)), now, Thresholds::default());
    let green = attention(&mk(Some(false)), now, Thresholds::default());
    Probe {
        name: "attention: idle-without-claim fires for a live session, not a dead one",
        red_fires: red.iter().any(|a| a.kind == "idle-without-claim"),
        green_passes: !green.iter().any(|a| a.kind == "idle-without-claim"),
    }
}

/// air-24e: a rule with a date in it goes quiet when the date passes. Both of Air's cutoffs
/// passed on 2026-08-23, seven tests went red because their fixtures had been written inside
/// the fallback window, and main stayed red six days because nothing watches a date. Red: a
/// clock past the cutoff reports it EXPIRED. Green: a clock before it reports it active.
///
/// The mutation that made it red: hardcoding `expired: false` in `doctor::dated_rules` — the
/// probe's red half then finds no expired rule.
fn probe_expired_cutoff_is_reported() -> Probe {
    use crate::cmd::doctor::dated_rules;
    // Two clocks that need no date of their own, so this probe holds no copy of a rule's
    // number: after every possible cutoff, and before every one Air will ever carry.
    let after = dated_rules(jiff::Timestamp::MAX);
    let before = dated_rules(jiff::Timestamp::UNIX_EPOCH);
    Probe {
        name: "doctor: a dated rule says so when its cutoff has passed",
        red_fires: !after.is_empty() && after.iter().all(|r| r.expired),
        green_passes: !before.is_empty() && before.iter().all(|r| !r.expired),
    }
}

/// air-8p4: a claim row survived `bd close`, so conditions kept firing on a bead that was
/// closed and landed. Red: the row still open, `handover-not-green` fires on it — the state
/// The adopter's coordinator spent a setup window diagnosing. Green: the close releases the row
/// and nothing fires; and `-s awaiting_review` does NOT release it, because a handed-over bead
/// is still the worker's until it lands (air-3eu).
///
/// The mutation that made it red, seen: widening `closes_bead` to `handover_bead(cmd)`, so
/// `-s awaiting_review` releases too — "red fires / green BLOCKED". The hook path that applies
/// it is covered separately by `hook::tests::a_successful_close_releases_the_claim_and_awaiting_review_does_not`,
/// whose mutation is deleting the arm from `dispatch`.
fn probe_close_releases_the_claim() -> Probe {
    use crate::cmd::hook::closes_bead;
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention};

    const NOW: &str = "2026-08-20T12:00:00Z";
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("zz-1", "w", &[], "2026-08-20T11:00:00Z")
            .map_err(|e| e.to_string())?;
        l.stamp_handover("zz-1", "w", "2026-08-20T11:50:00Z")
            .map_err(|e| e.to_string())?;
        // A live worker, recently seen, so the only thing that can speak is the claim.
        let fires = |l: &Ledger| -> Result<Vec<&'static str>, String> {
            let claims = l.open_claims().map_err(|e| e.to_string())?;
            let s = Snapshot {
                workers: vec![WorkerView {
                    worker: "w".into(),
                    role: "worker".into(),
                    green_at_head: Some(false),
                    claims,
                    session: Some(Session {
                        session_id: "s".into(),
                        state: "working".into(),
                        changed_at: "2026-08-20T11:59:00Z".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            Ok(attention(&s, NOW, Thresholds::default())
                .iter()
                .map(|a| a.kind)
                .collect())
        };
        let before = fires(&l)?;
        // Not an ending: a hand-over leaves the claim held.
        let handover_keeps_it = closes_bead("bd update zz-1 -s awaiting_review").is_none();
        // The close, as the PostToolUse arm applies it.
        let bead = closes_bead("bd close zz-1 --reason done").ok_or("close not recognised")?;
        let released = l
            .release_claim(&bead, "w", "closed", "t2")
            .map_err(|e| e.to_string())?;
        let after = fires(&l)?;
        // Threshold-independent on both sides: the claim on fd-1 is what speaks and what goes
        // quiet, so no fixture here is a second copy of a number in `Thresholds` (air-jc0).
        let still_held = l
            .open_claims()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|c| c.bead == "zz-1");
        Ok((
            before.contains(&"handover-not-green"),
            handover_keeps_it && released && !after.contains(&"handover-not-green") && !still_held,
        ))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "claim: a closed bead stops alarming; awaiting_review still holds it",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-0j4: a worker's HEAD is one sha, so every claim it holds is not-green for the same
/// reason and the same fix. The adopter's `air status` printed eleven `handover-not-green` lines
/// for one worker — one fact, eleven times.
///
/// Red: three stuck claims on one worker produce ONE line, and it names all three with a total.
/// Green: a single claim keeps its original wording, unchanged.
///
/// The mutation that made it red, seen: restoring the per-claim `out.push` loop — three lines
/// instead of one, so the red half's `len() == 1` fails.
fn probe_handover_not_green_is_one_line_per_worker() -> Probe {
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention};
    use air_ledger::claims::Claim;

    let claim = |bead: &str, at: &str| Claim {
        bead: bead.into(),
        worker: "w".into(),
        claimed_at: at.into(),
        declared_files: Vec::new(),
        first_handover_at: Some(at.into()),
        last_handover_at: Some(at.into()),
        handover_attempts: 1,
        released_at: None,
        release_reason: None,
    };
    let snap = |claims: Vec<Claim>| Snapshot {
        workers: vec![WorkerView {
            worker: "w".into(),
            role: "worker".into(),
            green_at_head: Some(false),
            claims,
            // A live worker seen a minute ago, so the claim is the only thing that can speak.
            session: Some(Session {
                session_id: "s".into(),
                state: "working".into(),
                changed_at: "2026-08-20T11:59:00Z".into(),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let now = "2026-08-20T12:00:00Z";
    let three = attention(
        &snap(vec![
            claim("zz-1", "2026-08-20T11:00:00Z"),
            claim("zz-2", "2026-08-20T11:30:00Z"),
            claim("zz-3", "2026-08-20T11:40:00Z"),
        ]),
        now,
        Thresholds::default(),
    );
    let one = attention(
        &snap(vec![claim("zz-1", "2026-08-20T11:00:00Z")]),
        now,
        Thresholds::default(),
    );
    Probe {
        name: "attention: three stuck claims on one worker are one line, not three",
        red_fires: three.len() == 1
            && three.first().is_some_and(|a| {
                a.kind == "handover-not-green"
                    && ["zz-1", "zz-2", "zz-3"]
                        .iter()
                        .all(|b| a.detail.contains(b))
                    && a.detail.contains("3 attempts in total")
            }),
        green_passes: one.len() == 1
            && one
                .first()
                .is_some_and(|a| a.detail.starts_with("zz-1 handed over 1 time(s)")),
    }
}

/// air-p61: `air status`'s bd budget was a flat 2 s, chosen before anything measured bd. bd's
/// measured p99 here is 1644 ms — 356 ms of headroom — and the adopter's MEDIAN is 1760 ms,
/// above the whole budget, so their status reconcile timed out on ordinary calls.
///
/// Red: at the adopter's measured median the budget rises above it, instead of sitting under it.
/// Green: it never exceeds the cap that keeps `air status` inside the MCP tool budget
/// (air-19u), and a ledger with no measurement yet keeps the old floor.
///
/// No number here is a second copy of a rule: the two inputs are measurements from the two
/// repos' event logs, and both assertions are relations (`>`, `<=`) rather than equalities
/// against a constant, so moving the multiplier cannot silently silence this (air-jc0).
fn probe_status_bd_budget_follows_the_measurement() -> Probe {
    use crate::cmd::bd_latency::status_bd_budget;
    let ms = |d: std::time::Duration| u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
    // Measured medians: the adopter 1760 ms over 260,601 calls; this repo 1430 ms over 495,892.
    let theirs = ms(status_bd_budget(Some(1760)));
    let ours = ms(status_bd_budget(Some(1430)));
    let cold = ms(status_bd_budget(None));
    // A pathological median must not push the budget into the channel's own budget.
    let awful = ms(status_bd_budget(Some(60_000)));
    Probe {
        name: "status: the bd budget is derived from bd's measured cost, not a constant",
        red_fires: theirs > 1760 && ours > 1430 && theirs > ours,
        green_passes: cold == 2_000 && awful <= 8_000 && awful > ours,
    }
}

/// air-q07: the cost the owner most wants minimised was the one the ledger did not contain.
/// `SendMessage` was not in the installed PreToolUse matcher, so counting agent-to-agent
/// traffic from the event log returned zero — not because there was none, but because it was
/// invisible.
///
/// Red: the installed matcher names `SendMessage`, and a `SendMessage` payload is recognised
/// as a message with its recipient and a byte count. Green: the audit sums it per worker, no
/// content is recorded anywhere, and nothing about it is a decision — the report is a report.
///
/// The mutation that made it red, seen: dropping `SendMessage` from `install::hook_entries`.
fn probe_agent_traffic_is_counted() -> Probe {
    use crate::cmd::audit::traffic_of;
    use crate::cmd::install::hook_entries;
    use air_hooks::HookInput;

    let matcher_covers = hook_entries()
        .iter()
        .any(|(event, m)| *event == "PreToolUse" && m.is_some_and(|m| m.contains("SendMessage")));
    let input = HookInput::parse(
        r#"{"session_id":"s","hook_event_name":"PreToolUse","tool_name":"SendMessage",
            "tool_input":{"to":"main","message":"hello there","summary":"greeting"}}"#,
    )
    .ok();
    let parsed = input.as_ref().and_then(HookInput::message_sent);
    // Two workers, one day, and a line that is not a message.
    let day = concat!(
        r#"{"at":"2026-08-29T01:00:00Z","worker":"alpha","command":"hook.PreToolUse","decision":"messaged","inputs":{"to":"main","bytes":100}}"#,
        "\n",
        r#"{"at":"2026-08-29T02:00:00Z","worker":"alpha","command":"hook.PreToolUse","decision":"messaged","inputs":{"to":"beta","bytes":40}}"#,
        "\n",
        r#"{"at":"2026-08-29T03:00:00Z","worker":"beta","command":"hook.PreToolUse","decision":"observed","inputs":{}}"#,
        "\n",
    );
    let t = traffic_of(&[("2026-08-29".to_string(), day.to_string())], "2026-08-29");
    let summed =
        matches!(t.as_slice(), [one] if one.worker == "alpha" && one.sent == 2 && one.bytes == 140);
    Probe {
        name: "traffic: SendMessage reaches the hook and the audit sums it per worker",
        red_fires: matcher_covers && parsed == Some(("main".to_string(), 11)),
        // No content anywhere: the parse returns a recipient and a length, never the text.
        green_passes: summed && !format!("{t:?}").contains("hello there"),
    }
}

/// air-srv (owner ruling 2026-09-05): every `SendMessage` is recorded, content included.
/// Agents solve problems together over `SendMessage` and none of it reached the ledger unless
/// someone captured it by hand. The event line is unchanged (recipient and bytes, air-q07);
/// the text goes to the `messages` table.
///
/// Red: one `SendMessage` hook input produces exactly one `messages` row carrying the content,
/// the recipient, and the sender the session row knows. Green: a second identical call
/// produces a second row, not a dedupe, and `summary` is on neither.
///
/// The mutation that made it red, seen: `let content = "";` in `hook::record_message`, which
/// is air-q07's content-free record put back. (Replacing the call in `pre_tool_use` with
/// `Ok(())` reaches the hook unit test instead, and was seen red there.)
fn probe_a_message_is_recorded_with_its_content() -> Probe {
    use crate::cmd::hook::record_message;
    use air_hooks::HookInput;

    let Ok(ledger) = Ledger::open_in_memory() else {
        return Probe {
            name: "messages: a SendMessage is one ledger row with its content",
            red_fires: false,
            green_passes: false,
        };
    };
    let session_row = ledger
        .conn()
        .execute(
            "INSERT INTO sessions (session_id, worker, state, changed_at, started_at, role, project) \
             VALUES ('s-msg','alpha','running','t','t','worker','air')",
            [],
        )
        .is_ok();
    let input = HookInput::parse(
        r#"{"session_id":"s-msg","hook_event_name":"PreToolUse","tool_name":"SendMessage",
            "tool_input":{"to":"main","message":"the plan is X","summary":"about X"}}"#,
    )
    .ok();
    let first = input
        .as_ref()
        .is_some_and(|i| record_message(&ledger, "alpha", i).is_ok());
    let after_one = ledger.messages().unwrap_or_default();
    let red_fires = session_row
        && first
        && matches!(after_one.as_slice(), [m]
            if m.content == "the plan is X" && m.to == "main" && m.bytes == 13
            && m.from_worker == "alpha" && m.from_role == "worker" && m.project == "air"
            && m.session_id == "s-msg");
    let second = input
        .as_ref()
        .is_some_and(|i| record_message(&ledger, "alpha", i).is_ok());
    let after_two = ledger.messages().unwrap_or_default();
    let green_passes = second
        && after_two.len() == 2
        && after_two.iter().all(|m| m.content == "the plan is X")
        && !format!("{after_two:?}").contains("about X");
    Probe {
        name: "messages: a SendMessage is one ledger row with its content",
        red_fires,
        green_passes,
    }
}

/// air-uef (owner ruling 2026-09-05): the owner inbox is gone. Two queues reached the owner,
/// worker prose with no id and no acceptance, and beads labelled `owner`; a capture sat a week
/// for a bead that already existed and was already labelled. One queue now, and it is beads.
///
/// Red: a fleet with open owner-labelled beads and no captures reports the owner's count on
/// the `ready:` line of `air status`, named as the owner's queue. Green: the same snapshot
/// raises no condition at all, and `owner-decision-waiting` exists in neither the kind list
/// nor the mechanism registry, so nothing can push it.
///
/// The mutation that made it red, seen: the ready line's differ-branch replaced by
/// `String::new()`, which is the count silently gone.
fn probe_owner_queue_is_the_ready_line_not_a_condition() -> Probe {
    use crate::cmd::mechanisms::MECHANISMS;
    use crate::cmd::status::{Snapshot, Thresholds, attention, kinds, render_for_probe};

    const NOW: &str = "2026-09-05T12:00:00Z";
    let s = Snapshot {
        at: NOW.to_string(),
        ready_depth: Some(3),
        claimable_depth: Some(1),
        ..Default::default()
    };
    let text = render_for_probe(&s);
    let red_fires = text.contains("ready: 3 (1 claimable; 2 owner-labelled: the owner's queue");
    let gone = "owner-decision-waiting";
    let green_passes = attention(&s, NOW, Thresholds::default()).is_empty()
        && !kinds::ALL.contains(&gone)
        && !MECHANISMS.iter().any(|m| m.id == gone);
    Probe {
        name: "status: the owner's queue is the owner-labelled count on the ready line, and no condition",
        red_fires,
        green_passes,
    }
}

/// air-q9c: a lease defect is a signal for whoever WANTS the resource, and never for the
/// holder — who knows they hold it and was being told to break the thing they were using.
/// The adopter saw six of those in a day while the simulator and API were genuinely running.
///
/// Red: a defective lease with someone waiting fires, addressed to the WAITER, and tells them
/// the action is theirs. Green: the same defect with nobody waiting is silent (`lease take`
/// takes a defective lease on its own, so there is nobody to tell), and so is one where the
/// only name waiting is the holder's.
fn probe_lease_defect_reaches_the_waiter() -> Probe {
    use crate::cmd::status::{Snapshot, Thresholds, attention};
    use air_ledger::leases::Lease;

    let dead = || {
        vec![(
            Lease {
                resource: "runtime".into(),
                worker: "a".into(),
                session_id: None,
                pid: Some(1),
                pid_started: None,
                reason: "api".into(),
                taken_at: "2026-08-20T11:00:00Z".into(),
                heartbeat_at: "2026-08-20T11:00:00Z".into(),
            },
            Some("dead (pid 1 gone)".to_string()),
        )]
    };
    let snap = |wants: &[(&str, &[&str])]| Snapshot {
        leases: dead(),
        lease_wants: wants
            .iter()
            .map(|(r, who)| {
                (
                    (*r).to_string(),
                    who.iter().map(|w| (*w).to_string()).collect(),
                )
            })
            .collect(),
        ..Default::default()
    };
    let now = "2026-08-20T12:00:00Z";
    let waited = attention(&snap(&[("runtime", &["b"])]), now, Thresholds::default());
    let nobody = attention(&snap(&[]), now, Thresholds::default());
    let self_only = attention(&snap(&[("runtime", &["a"])]), now, Thresholds::default());
    Probe {
        name: "lease: a defect reaches the waiter, never the holder, and nobody waiting is silent",
        red_fires: waited.len() == 1
            && waited.first().is_some_and(|x| {
                x.kind == "lease-held-by-dead-session"
                    && x.worker == "b"
                    && x.detail.contains("yours to take now")
            }),
        green_passes: nobody.is_empty() && self_only.is_empty(),
    }
}

/// air-njb: surface notices are the only thing that tells an adopting repo what an upgrade
/// does to it, and the 2026-08-29 round changed more than any before it while adding one
/// notice.
///
/// Red: a repo at YESTERDAY's surface is told, and specifically about the installer defect —
/// a matcher installed before today was never updated by a re-run. Green: a repo already told
/// about everything is told nothing. **The empty case is what makes the non-empty one mean
/// something**: a diff that never goes quiet reports on every install and is ignored by the
/// second week.
///
/// Yesterday's set is derived from each notice's own `since` date, not from a list of ids
/// copied here — a copied list would stop being yesterday's the next time anyone appends
/// (air-jc0). The anchor day is fixed at the incident, and what yesterday's repo is told is
/// every notice dated ON OR AFTER it: the first version said "dated today" and went red the
/// day air-srv appended a notice dated a week later (2026-09-05), which is the same drift one
/// level up.
fn probe_yesterdays_repo_is_told_and_a_current_one_is_not() -> Probe {
    use crate::cmd::install::{SURFACE, surface_diff};

    const TODAY: &str = "2026-08-29";
    let ids = |f: fn(&str) -> bool| -> Vec<String> {
        SURFACE
            .iter()
            .filter(|c| f(c.since))
            .map(|c| c.id.to_string())
            .collect()
    };
    let yesterday = ids(|since| since < TODAY);
    let everything = ids(|_| true);

    let told = surface_diff(&yesterday);
    let quiet = surface_diff(&everything);
    // Every notice dated today or later, and nothing else, is what yesterday's repo has not
    // seen.
    let todays: Vec<&str> = SURFACE
        .iter()
        .filter(|c| c.since >= TODAY)
        .map(|c| c.id)
        .collect();
    Probe {
        name: "install: a repo at yesterday's surface is told what changed; a current one is told nothing",
        red_fires: !todays.is_empty()
            && told.len() == todays.len()
            && todays.iter().all(|id| told.iter().any(|c| c.id == *id))
            && told
                .iter()
                .any(|c| c.id == "install-refreshes-matcher" && c.silent_break),
        green_passes: quiet.is_empty(),
    }
}

/// air-w9d: `air install` goes forward only. Owner, 2026-08-29: *"only allow upgrades, not
/// downgrades."*
///
/// The forward diff has always existed; nothing computed the reverse, so an older `air` wrote
/// over a newer repo's record and reported success. That is the trap under all twelve upgrade
/// notices — they are printed BY `air install`, so a stale binary shows an adopting repo none
/// of them, including the one telling them to check their binary.
///
/// **Three directions, not two.** Red: a binary BELOW the recorded surface version is refused.
/// Green: equal writes, higher writes, and a repo with no version recorded at all writes —
/// that last one is every repo running Air today, the adopter included, and refusing it would
/// lock them all out. The silent cases are what make the refusal mean anything.
///
/// The first version of this compared notice-id SETS. A set says "different", never "behind",
/// so a worker installing from its own branch made a later main-built binary look older than
/// the repo. The version is a total order and the question does not arise.
fn probe_install_goes_forward_only() -> Probe {
    use crate::cmd::install::{SURFACE_VERSION, may_install};

    let here = SURFACE_VERSION;
    let older = here.saturating_sub(1);
    let newer = here.saturating_add(1);
    Probe {
        name: "install: a binary below the repo's surface version is refused; equal, higher and unrecorded write",
        // A real downgrade only exists once the version has moved at least once.
        red_fires: here > 0 && !may_install(older, Some(here)),
        green_passes: may_install(here, Some(here))
            && may_install(newer, Some(here))
            && may_install(here, None)
            && may_install(older, None),
    }
}

/// The channel pushes a new condition once and not again until it escalates.
fn probe_channel_dedupe() -> Probe {
    use crate::cmd::mcp::{Pushed, select_new};
    use crate::cmd::status::Attention;
    let a = |m: i64| Attention {
        worker: "w".into(),
        kind: "idle-with-claim",
        detail: String::new(),
        for_minutes: m,
        fingerprint: String::new(),
    };
    let mut p = Pushed::new();
    let first = select_new(&mut p, &[a(5)]).len() == 1;
    let quiet = select_new(&mut p, &[a(6)]).is_empty();
    Probe {
        name: "channel: new condition pushed once, repeat suppressed",
        red_fires: first,
        green_passes: quiet,
    }
}

/// air-b5k: adopting-air.md step 5 told a repo to hand-edit a `bd prime --hook-json` hook
/// out of `.claude/settings.json`, because `air install` merges and never removes it. A hand
/// edit in an adoption walkthrough is the step that gets skipped once and never revisited, and
/// nothing reported its state afterwards. The installer already reads that file.
///
/// Red: a settings file with the stale hook, after Air's merge, is reported with its event,
/// its command, and what to do. Green: Air's own hooks and a bd hook that is not `prime`
/// report nothing, so the line means something when it appears.
///
/// The mutation that made it red, seen: `stale_hooks` matching no command (`is_bd` forced
/// false), which is the installer back to silent.
fn probe_install_reports_a_stale_bd_prime_hook() -> Probe {
    use crate::cmd::install::{merge_hooks, render_stale, stale_hooks};

    let with = merge_hooks(serde_json::json!({"hooks": {"SessionStart": [{"hooks": [
        {"type": "command", "command": "bd prime --hook-json"}]}]}}));
    let stale = stale_hooks(&with);
    let text = render_stale(&stale);
    let red_fires = stale.len() == 1
        && stale
            .first()
            .is_some_and(|h| h.event == "SessionStart" && h.command == "bd prime --hook-json")
        && text.contains("STALE HOOK: SessionStart runs `bd prime --hook-json`")
        && text.contains("do: ");
    let without = merge_hooks(serde_json::json!({"hooks": {"Stop": [{"hooks": [
        {"type": "command", "command": "bd ready --json"}]}]}}));
    let green_passes = stale_hooks(&without).is_empty()
        && stale_hooks(&merge_hooks(serde_json::json!({}))).is_empty()
        && render_stale(&[]).is_empty();
    Probe {
        name: "install: a stale `bd prime` hook is reported with its fix; Air's hooks and other bd hooks are not",
        red_fires,
        green_passes,
    }
}

/// air-80x.3: the verify lane needs one fact, which branches to merge into the next batch,
/// and it lived in messages; in the adopter's 2026-08-29 round the batch never formed. The rule
/// is three lookups: head contains main, no green at that head, a `Bead:` trailer names a
/// bead the worker holds.
///
/// Red: a branch that merged main and committed a claimed bead is listed, on the status line
/// with its beads. Green: the same branch with a green at its head is absent (it is landable,
/// nothing to batch); a branch behind main is absent; a branch naming only an unclaimed bead
/// is absent; and every absence carries the first fact it lacks.
///
/// The mutation that made it red, seen: `batch_ready_rule` dropping the `green_at_head` arm,
/// which lists a landable branch for a batch it does not need.
fn probe_batch_ready_is_a_fact_with_three_parts() -> Probe {
    use crate::cmd::status::{BatchFacts, Snapshot, batch_ready_rule, render_for_probe};

    let base = BatchFacts {
        worker: "alpha".into(),
        head: "abcdef1234567890".into(),
        contains_main: true,
        green_at_head: false,
        carried: vec!["zz-1".into(), "zz-9".into()],
        held: vec!["zz-1".into()],
    };
    let ready = batch_ready_rule(&base);
    let line = render_for_probe(&Snapshot {
        batch_ready: ready.clone().into_iter().collect(),
        ..Default::default()
    });
    let red_fires = ready
        .as_ref()
        .is_ok_and(|b| b.worker == "alpha" && b.beads == ["zz-1"])
        && line.contains("batch-ready: alpha at abcdef12 (zz-1)\n");

    let green = batch_ready_rule(&BatchFacts {
        green_at_head: true,
        ..base.clone()
    });
    let behind = batch_ready_rule(&BatchFacts {
        contains_main: false,
        ..base.clone()
    });
    let unclaimed = batch_ready_rule(&BatchFacts {
        held: vec![],
        ..base.clone()
    });
    let green_passes = green.as_ref().is_err_and(|n| n.check == "green-at-head")
        && behind.as_ref().is_err_and(|n| n.check == "behind-main")
        && unclaimed
            .as_ref()
            .is_err_and(|n| n.check == "no-claimed-bead" && n.detail.contains("zz-1 zz-9"))
        && !render_for_probe(&Snapshot::default()).contains("batch-ready");
    Probe {
        name: "status: batch-ready is three facts (contains main, no green at head, a claimed bead named); a green or behind branch is absent with its reason",
        red_fires,
        green_passes,
    }
}

/// `air install` merge adds our hooks to an empty config and changes nothing the second time.
fn probe_install_merge() -> Probe {
    use crate::cmd::install::merge_hooks;
    let once = merge_hooks(serde_json::json!({}));
    let added = once
        .get("hooks")
        .and_then(|h| h.get("Stop"))
        .is_some_and(Value::is_array);
    let idempotent = merge_hooks(once.clone()) == once;
    Probe {
        name: "install: hook merge adds once, idempotent after",
        red_fires: added,
        green_passes: idempotent,
    }
}

fn base_facts() -> GateFacts {
    GateFacts {
        worker: "probe".into(),
        head: "0123456789abcdef".into(),
        green_at_head: true,
        tree_green: None,
        batch_green: None,
        batch_predates: None,
        last_green_sha: None,
        main_is_ancestor: true,
        main_sha: "fedcba9876543210".into(),
        main_moved: None,
        bead_claimed_or_carried: true,
        runs_at_head: (1, 0),
        digest_present: None,
        digest_dir: None,
        bead: None,
        held_beads: vec![],
        carried_beads: vec![],
        advisory: false,
    }
}

fn probe_gate_verify() -> Probe {
    let green = handover_verdict(&base_facts()).pass;
    let mut f = base_facts();
    f.green_at_head = false;
    let red = handover_verdict(&f).block;
    Probe {
        name: "gate: verify-green-at-head",
        red_fires: red,
        green_passes: green,
    }
}

fn probe_gate_main() -> Probe {
    let green = handover_verdict(&base_facts()).pass;
    let mut f = base_facts();
    f.main_is_ancestor = false;
    let red = handover_verdict(&f).block;
    Probe {
        name: "gate: main-merged",
        red_fires: red,
        green_passes: green,
    }
}

/// air-4up: eight refusals in one adopter's round, all caused by a coordinator landing, every
/// one worded as a defect in the worker's tree. Red: with a landing on record that HEAD does
/// not contain, the refusal names it — the sha, how long ago, whose branch — and keeps the
/// phrase the adopter's counts refusals by. Green: the fix is unchanged, and with no landing to
/// name the gate still refuses and names main without inventing a cause.
fn probe_gate_names_the_landing_that_moved_main() -> Probe {
    use air_hooks::MainMove;

    let mut f = base_facts();
    f.main_is_ancestor = false;
    f.main_moved = Some(MainMove {
        merge_commit: "abcdef0123456".into(),
        worker: "lane".into(),
        at: "t".into(),
        ago_secs: Some(40),
    });
    let v = handover_verdict(&f);
    let red = v.block
        && v.message
            .contains("moved 40s ago to abcdef0 (landing from lane)")
        && v.message.contains("main is not an ancestor of HEAD");
    let fix_unchanged = v.missing.iter().any(|m| {
        m.check == "main-merged" && m.fix == "git merge main && air record verify -- make verify"
    });
    let mut g = base_facts();
    g.main_is_ancestor = false;
    let plain = handover_verdict(&g);
    Probe {
        name: "gate: a refusal after a landing names the landing that moved main, when and from whom; the fix is unchanged",
        red_fires: red,
        green_passes: fix_unchanged
            && plain.block
            && !plain.message.contains("landing")
            && plain.message.contains("main is at fedcba9"),
    }
}

/// air-5wq: `handover ok: w2 at 3c39883` reads as a clearance and is a snapshot. The adopter
/// measured 88 refusals in four days within 120 s of that worker's own ok line. Red: the ok
/// line names the main it checked against. Green: after main moves, the refusal names the new
/// main and not the old one, so the pair reads as main having moved; an unreadable main is
/// omitted rather than rendered empty.
fn probe_handover_ok_names_the_main_it_checked() -> Probe {
    let ok = handover_verdict(&base_facts());
    let red = ok.pass && ok.message.ends_with(", containing main fedcba9");
    let mut moved = base_facts();
    moved.main_is_ancestor = false;
    moved.main_sha = "1111111222222".into();
    let refused = handover_verdict(&moved);
    let mut unknown = base_facts();
    unknown.main_sha = String::new();
    Probe {
        name: "handover: the ok line names the main it checked against, and a refusal after main moves names the new one",
        red_fires: red,
        green_passes: refused.block
            && refused.message.contains("main is at 1111111")
            && !refused.message.contains("fedcba9")
            && handover_verdict(&unknown).message == "handover ok: probe at 0123456",
    }
}

/// air-75u: the main checkout's Stop hook reported a hand-over refusal naming w1's HEAD, twice
/// in one adopter's round, because the hook took its identity from the shell's directory and
/// the coordinator's shell was in w1's worktree. Red: with the launcher's `AIR_ROLE`, the
/// session is main whatever the shell says, and a worker is its `BEADS_ACTOR`. Green: a
/// session Air did not launch is what its checkout says, and a refusal names whose tree it is
/// about, so the un-launched case is at least scoped.
fn probe_session_identity_is_the_launchers() -> Probe {
    use crate::cmd::hook::identity_from;

    let red = identity_from(Some("coordinator"), None, "w1") == "main"
        && identity_from(Some("worker"), Some("w2"), "w1") == "w2";
    let mut f = base_facts();
    f.main_is_ancestor = false;
    let v = handover_verdict(&f);
    Probe {
        name: "hook: a session is who its launcher says, not where its shell sits; a refusal names whose tree it is about",
        red_fires: red,
        green_passes: identity_from(None, Some("tester"), "w1") == "w1"
            && identity_from(None, None, "main") == "main"
            && v.message
                .starts_with("handover refused for probe at 0123456: "),
    }
}

/// `air <word>` mentions in COMMAND position in one source file's non-comment lines: after a
/// backtick, an opening paren, or a colon-space, which is how every shipped advice string
/// names a command ("run `air status`", "(air claim fd-1)", "fix: air record verify"). Prose
/// about Air in a string ("the newer air and re-run", "will add air hooks") is preceded by a
/// plain space or opens the literal, and is not a command. Returns (line, word).
fn command_mentions(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let bytes = line.as_bytes();
        let mut from = 0;
        while let Some(pos) = line.get(from..).and_then(|s| s.find("air ")) {
            let at = from.saturating_add(pos);
            from = at.saturating_add(4);
            let before = at.checked_sub(1).and_then(|p| bytes.get(p)).copied();
            let two_before = at.checked_sub(2).and_then(|p| bytes.get(p)).copied();
            let command_position = matches!(before, Some(b'`') | Some(b'('))
                || (before == Some(b' ') && two_before == Some(b':'));
            if !command_position {
                continue;
            }
            let word: String = line
                .get(from..)
                .unwrap_or("")
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                .collect();
            if !word.is_empty() {
                out.push((i.saturating_add(1), word));
            }
        }
    }
    out
}

/// air-w91: the file-overlap advice told every worker to run `air peer <name>`, a command that
/// was planned in CLAUDE.md's subsystem table and never built; the adopter's w3 hit
/// "unrecognized subcommand" while already dealing with a shared file. `gc` from the same
/// list shipped; `peer` did not; the string went out anyway, into every repo Air installs into.
///
/// Red: the pre-fix string is caught (`peer` is no subcommand). Green: every `air <word>` in
/// command position in the shipped sources resolves to a subcommand clap knows, so the next
/// planned-but-unbuilt command cannot ship in advice again. The command list is read from the
/// binary's own clap tree, never from a list somebody typed.
fn probe_shipped_advice_names_real_subcommands() -> Probe {
    use clap::CommandFactory;

    let real: Vec<String> = crate::Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    let sources: &[(&str, &str)] = &[
        ("hook.rs", include_str!("hook.rs")),
        ("status.rs", include_str!("status.rs")),
        ("install.rs", include_str!("install.rs")),
        ("land.rs", include_str!("land.rs")),
        ("claim.rs", include_str!("claim.rs")),
        ("capture.rs", include_str!("capture.rs")),
        ("lease.rs", include_str!("lease.rs")),
        ("handover.rs", include_str!("handover.rs")),
        ("record.rs", include_str!("record.rs")),
        ("close.rs", include_str!("close.rs")),
        ("launch.rs", include_str!("launch.rs")),
        ("init.rs", include_str!("init.rs")),
        ("doctor.rs", include_str!("doctor.rs")),
        ("mcp.rs", include_str!("mcp.rs")),
        ("gate.rs", include_str!("../../../hooks/src/gate.rs")),
    ];
    let dangling: Vec<String> = sources
        .iter()
        .flat_map(|(name, src)| {
            command_mentions(src)
                .into_iter()
                .filter(|(_, w)| !real.contains(w))
                .map(move |(line, w)| format!("{name}:{line} names `air {w}`"))
        })
        .collect();
    if !dangling.is_empty() {
        eprintln!("selftest: shipped advice names a subcommand that does not exist:");
        for d in &dangling {
            eprintln!("  {d}");
        }
    }
    let pre_fix = "context: Some(format!(\"... (run `air peer <name>` for their green sha)\"))";
    let red = command_mentions(pre_fix)
        .iter()
        .any(|(_, w)| w == "peer" && !real.contains(w));
    Probe {
        name: "hook: every `air <subcommand>` a shipped hook or status string names is a real subcommand",
        red_fires: red,
        green_passes: !real.is_empty() && dangling.is_empty(),
    }
}

/// air-an9: `status.rs`'s unit tests carried three literal timestamps chosen to sit either
/// side of `Thresholds::default()` at the time they were written, the shape that made main
/// red for six days when two dated cutoffs expired (air-24e), and the shape air-jc0 took out
/// of the probes. They now hold ONE instant and derive every age from the thresholds. Red: a
/// second literal instant in the file is caught. Green: the file has exactly one, and the
/// helpers that derive the ages read the thresholds (the control air-jc0 prescribes, moving a
/// default and watching the tests stay green, is run by hand and recorded in the digest).
fn probe_status_tests_hold_one_instant() -> Probe {
    let src = include_str!("status.rs");
    let instants = src.matches("\"2026-08-20T").count();
    Probe {
        name: "status: the unit tests hold one instant and derive every age from Thresholds::default()",
        red_fires: instants == 1,
        green_passes: src.contains("fn past_every_line_min() -> i64")
            && src.contains("fn under_every_line_min() -> i64")
            && src.contains("fn every_line() -> [i64; 4]"),
    }
}

fn probe_handover_matcher() -> Probe {
    Probe {
        name: "hook: handover command matcher",
        red_fires: is_handover_command("bd close zz-1")
            && is_handover_command("bd update zz-1 -s awaiting_review"),
        green_passes: !is_handover_command("git commit -am wip")
            && !is_handover_command("bd update zz-1 --claim"),
    }
}

fn probe_ledger_roundtrip() -> Probe {
    let ok = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let run = VerifyRun {
            id: new_id(),
            worker: "probe".into(),
            sha: "abc".into(),
            kind: Kind::Verify,
            exit_code: 0,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: "2026-01-01T00:00:00Z".into(),
            finished_at: "2026-01-01T00:00:00Z".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: None,
            members: vec![],
        };
        l.record_verify(&run).map_err(|e| e.to_string())?;
        let green = l
            .green_at("abc", None, Kind::Verify)
            .map_err(|e| e.to_string())?
            .is_some();
        let red = l
            .green_at("zzz", None, Kind::Verify)
            .map_err(|e| e.to_string())?
            .is_none();
        Ok((red, green))
    })();
    let (red, green) = ok.unwrap_or_else(blocked);
    Probe {
        name: "ledger: verify_runs round-trip",
        red_fires: red,
        green_passes: green,
    }
}

/// air-ppm: a run killed by signal records no verdict, and a genuine exit-2 failure still
/// records red. The second is what makes the first safe.
///
/// Red: `run_tee` on a child that dies by SIGTERM yields 143, not -1, and a 143 row at a sha
/// is not the latest run there, not red, not one side of a flaky pair, and does not turn a
/// green tree red. Green: an exit-2 row is red and does count toward flakiness beside a green.
fn probe_killed_is_no_verdict() -> Probe {
    use crate::cmd::record::run_tee;
    use air_ledger::verify::{KILLED_EXITS, Verdict};

    let dir = std::env::temp_dir();
    // The real signal path: the child kills itself with TERM, and Air sees 128 + 15.
    let signalled = run_tee("sh", &["-c".into(), "kill -TERM $$".into()], &dir)
        .map(|(code, _)| code)
        .unwrap_or(-1);
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let run = |sha: &str, exit: i32, at: &str| VerifyRun {
            id: new_id(),
            worker: "probe".into(),
            sha: sha.into(),
            kind: Kind::Verify,
            exit_code: exit,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: at.into(),
            finished_at: at.into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: Some("T".into()),
            members: vec![],
        };
        l.record_verify(&run("aaa", 0, "t1"))
            .map_err(|e| e.to_string())?;
        l.record_verify(&run("aaa", signalled, "t2"))
            .map_err(|e| e.to_string())?;
        let killed_is_no_verdict = KILLED_EXITS.contains(&signalled)
            && run("aaa", signalled, "t").verdict() == Verdict::Killed
            && l.green_at("aaa", None, Kind::Verify)
                .map_err(|e| e.to_string())?
                .is_some()
            && l.runs_at("aaa", Kind::Verify).map_err(|e| e.to_string())? == (1, 0)
            && l.green_at("bbb", Some("T"), Kind::Verify)
                .map_err(|e| e.to_string())?
                .is_some();
        // A kill alone at a commit is nothing at all.
        l.record_verify(&run("ccc", 137, "t3"))
            .map_err(|e| e.to_string())?;
        let alone = l
            .latest_run_at_commit("ccc", Kind::Verify)
            .map_err(|e| e.to_string())?
            .is_none();
        // A genuine failure is red, and flaky beside the green.
        l.record_verify(&run("aaa", 2, "t4"))
            .map_err(|e| e.to_string())?;
        let exit_2_is_red = run("aaa", 2, "t").verdict() == Verdict::Red
            && l.green_at("aaa", None, Kind::Verify)
                .map_err(|e| e.to_string())?
                .is_none()
            && l.runs_at("aaa", Kind::Verify).map_err(|e| e.to_string())? == (1, 1);
        Ok((killed_is_no_verdict && alone, exit_2_is_red))
    })()
    .unwrap_or_else(blocked);
    let (red, green) = res;
    Probe {
        name: "record: a run killed by signal (143/137) records no verdict at its sha; an exit-2 failure is still red",
        red_fires: red,
        green_passes: green,
    }
}

/// air-7wf, both directions, on a real landing. A branch head is recorded green with its
/// tree; `commit-tree` builds the landing commit off main from that tree, exactly as `air land`
/// does (air-odv). Under `verify_key: tree` the landing reads green with no new verify. Under
/// the default `commit` key it does not, and the display names the tree green it is declining.
/// A commit over a tree nobody verified is not green under either key.
fn probe_green_follows_the_tree_only_where_declared() -> Probe {
    use crate::cmd::green::{Key, at_under, tree_of};

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "main"])?;
        let main = g(&["rev-parse", "HEAD"])?;
        std::fs::write(dir.join("f"), "work").map_err(|e| e.to_string())?;
        g(&["add", "f"])?;
        g(&["commit", "-q", "-m", "branch work"])?;
        let branch = g(&["rev-parse", "HEAD"])?;
        let tree = tree_of(&dir, &branch).map_err(|e| e.to_string())?;
        // The landing commit, built the way `air land` builds it: a new sha, the same tree.
        let landing = g(&[
            "commit-tree",
            &tree,
            "-p",
            &main,
            "-p",
            &branch,
            "-m",
            "Land",
        ])?;
        // A commit over a tree nobody verified.
        std::fs::write(dir.join("f"), "other").map_err(|e| e.to_string())?;
        g(&["commit", "-q", "-am", "unverified"])?;
        let unverified = g(&["rev-parse", "HEAD"])?;

        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_verify(&VerifyRun {
            id: new_id(),
            worker: "probe".into(),
            sha: branch.clone(),
            kind: Kind::Verify,
            exit_code: 0,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: "t".into(),
            finished_at: "t".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: Some(tree.clone()),
            members: vec![],
        })
        .map_err(|e| e.to_string())?;

        let under = |sha: &str, key: Key| at_under(&l, &dir, sha, Kind::Verify, key);
        // Green: the landing reads green from its tree where the repo declares it, and the
        // branch head reads green at its commit under either key.
        let landed = under(&landing, Key::Tree)?;
        let green = landed.holds()
            && landed.line().starts_with("green (same tree as")
            && under(&branch, Key::Commit)?.holds()
            && under(&branch, Key::Tree)?.holds();
        // Red: the same landing is NOT green by default, and says why; a tree nobody verified
        // is not green under either key.
        let declined = under(&landing, Key::Commit)?;
        let red = !declined.holds()
            && declined
                .line()
                .starts_with("not green (this exact tree is green at")
            && !under(&unverified, Key::Tree)?.holds()
            && !under(&unverified, Key::Commit)?.holds();
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "green: a landing reads green from its tree only where the repo declares verify_key tree; an unverified tree never does",
        red_fires: red,
        green_passes: green,
    }
}

/// Real `git merge-base --is-ancestor` on a scratch repo: proves the spawn path and the
/// exit-code interpretation (0 yes / 1 no).
fn probe_git_ancestor() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let a = g(&["rev-parse", "HEAD"])?;
        g(&["checkout", "-q", "-b", "wt"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "b"])?;
        let b = g(&["rev-parse", "HEAD"])?;
        let yes = crate::git::is_ancestor(&dir, &a, &b).map_err(|e| e.to_string())?;
        let no = crate::git::is_ancestor(&dir, &b, &a).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok((!no, yes))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "git: is-ancestor exit codes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-er0: the `--task` text must not be in the worker process's command line. The adopter's
/// seven worker deaths of 2026-08-30 were `pkill -f "air record verify"` matching the prompt
/// in every peer's argv (`ps -o command=` showed the whole task). Red: the old shape, the
/// task pushed into argv, carries the distinctive string. Green: `air worker --task` launched
/// against a stub `claude` (`AIR_CLAUDE_BIN`, which records the argv it was exec'd with,
/// which is what `ps -o command=` shows) has NO element containing that string, and the task
/// still reaches the session as its first prompt: argv opens on the sentence naming
/// `.air/tasks/w.md`, that sentence is read as the prompt and not as a deny value (air-2ct),
/// and the file holds the task byte for byte. Without the second half the fix would be a
/// silent no-op and every worker would start idle.
fn probe_worker_task_prompt() -> Probe {
    use crate::cmd::launch::{task_is_prompt, task_prompt, worker_argv};
    let task = "say hello, it's $HOME, then run air record verify -- make verify";
    let marker = "air record verify";
    let mut old = worker_argv("w", "air", std::path::Path::new("/r/roles.md"), &[]);
    old.push(task.to_string());
    let red = old.iter().any(|a| a.contains(marker));

    let green = (|| -> Result<bool, String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // Canonical, because the launcher names the task file from its cwd as the kernel
        // reports it (`/private/var/...` on macOS, not the `/var/...` temp_dir hands out).
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        for args in [
            &["init", "-q", "-b", "main"][..],
            // A commit, because the launcher now creates the lane's worktree first (air-fdz)
            // and a worktree needs something to branch from.
            &["commit", "-q", "--allow-empty", "-m", "a"][..],
        ] {
            let git = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !git.status.success() {
                return Err(String::from_utf8_lossy(&git.stderr).to_string());
            }
        }
        // The stub records its argv in a file rather than on stdout: without a tty (this
        // probe under `air record verify`, a Bash tool) the launcher starts the stub inside a
        // detached tmux session (air-tdc), where stdout is the pane. With a tty it execs
        // the stub directly. Either way the file appears; the socket keeps tmux private.
        let stub = dir.join("claude-stub");
        let argv_file = dir.join("argv");
        let cwd_file = dir.join("cwd");
        // The cwd as well as the argv, since air-8gj: `--worktree` is gone from the line and
        // the worktree IS the cwd, so the argv alone can no longer show the isolation.
        // Written first; the argv file is still the readiness signal.
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\npwd > {}\nprintf '%s\\0' \"$@\" > {}.tmp && mv {}.tmp {}\n",
                cwd_file.display(),
                argv_file.display(),
                argv_file.display(),
                argv_file.display()
            ),
        )
        .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        let socket = format!("air-selftest-{}", new_id());
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let out = air_command(&exe, &dir)
            .env("AIR_CLAUDE_BIN", &stub)
            .env("AIR_TMUX_SOCKET", &socket)
            .env_remove("AIR_TMUX_MODE")
            .args(["worker", "w", "--task", task])
            .output()
            .map_err(|e| e.to_string())?;
        let mut raw = None;
        // Up to 60 s (air-g7e, was 10): a fresh executable's first exec takes seconds on
        // macOS and tens of seconds at load 186. The deadline is not what either probe
        // is about, and a short one turns a busy machine into a red with no reason given.
        for _ in 0..6000 {
            if let Ok(b) = std::fs::read(&argv_file) {
                raw = Some(b);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = Command::new("tmux")
            .args(["-L", &socket, "kill-server"])
            .output();
        let task_path = dir.join(".air").join("tasks").join("w.md");
        let on_disk = std::fs::read_to_string(&task_path).unwrap_or_default();
        let worktree = dir.join(".claude").join("worktrees").join("w");
        let worktree_made = worktree.join(".git").is_file();
        let ran_in = std::fs::read_to_string(&cwd_file).unwrap_or_default();
        let ran_in_worktree = ran_in.trim() == worktree.to_string_lossy();
        let _ = std::fs::remove_dir_all(&dir);
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        let raw = raw.ok_or_else(|| "stub never ran".to_string())?;
        let argv: Vec<String> = String::from_utf8_lossy(&raw)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let prompt = task_prompt(&task_path);
        // air-fdz: the lane's worktree exists before claude runs. air-8gj: claude is no longer
        // HANDED it by name — it is started IN it, and the cwd is what holds the lane there.
        // Both halves are asserted, because dropping the flag without the cwd would be a
        // worker running loose in the main checkout.
        Ok(!argv.iter().any(|a| a.contains(marker))
            && argv.first().is_some_and(|a| *a == prompt)
            && task_is_prompt(&argv, &prompt)
            && on_disk == format!("{task}\n")
            && worktree_made
            && ran_in_worktree
            && !argv.iter().any(|a| a == "--worktree"))
    })()
    .unwrap_or(false);
    Probe {
        name: "launch: --task reaches claude as the prompt by file; the task text is not in argv, and claude runs in the worktree Air made rather than being handed it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-9dg: Air's env has to reach the HOOK, because that is where the one refusal runs.
/// The adopter's workers carried two `--settings`; the second replaced the first, `AIR_ENFORCE`
/// never reached a hook, and `bd close` without a green was ADVISED and executed for five
/// hours. `probe_enforced_gate` sets the env directly and so never exercised delivery.
///
/// Red: the old shape. Two `--settings` on the line, the env the session runs with is the
/// second's alone, and the real `air hook` run in that env ALLOWS `bd close` on a claimed bead
/// with no green at HEAD (exit 0: advisory). Green: `air worker --task … -- --settings '{…}'`
/// launched against a stub `claude` that records its argv and its environment. The argv
/// carries ONE `--settings`, merged (theirs kept, AIR_ENFORCE=1 on top); the environment the
/// stub ran in carries AIR_ENFORCE=1 and BEADS_ACTOR by `tmux new-session -e`, with the
/// launcher's own inherited values scrubbed so only delivery can put them there; and the real
/// `air hook`, run in exactly that recorded environment in a worktree holding a claim and no
/// green, REFUSES the close (exit 2) naming `air record verify`.
fn probe_env_reaches_the_hook() -> Probe {
    use crate::cmd::launch::worker_argv;
    let theirs = r#"{"remoteControlAtStartup":false}"#;
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        // A linked worktree, so the hook's worker is `w` with the worker role and its HEAD
        // contains main; the only thing missing for a close is the green.
        let wt = dir.join(".claude").join("worktrees").join("w");
        g(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "w",
            &wt.display().to_string(),
        ])?;
        let l = Ledger::open_for_repo(&dir).map_err(|e| e.to_string())?;
        l.record_claim("zz-1", "w", &[], "t0")
            .map_err(|e| e.to_string())?;
        drop(l);
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let hook = |env: &[(String, String)]| -> Result<(i32, String), String> {
            use std::io::Write;
            let input = serde_json::json!({
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {"command": "bd close zz-1 --reason done"},
                "session_id": "air-9dg-probe",
                "cwd": wt.display().to_string(),
            });
            let mut child = air_command(&exe, &wt)
                .arg("hook")
                .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(input.to_string().as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            Ok((
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).to_string(),
            ))
        };
        let env_of = |blob: &str| -> Vec<(String, String)> {
            serde_json::from_str::<serde_json::Value>(blob)
                .ok()
                .and_then(|v| v.get("env").and_then(|e| e.as_object()).cloned())
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default()
        };

        // RED: two --settings, the second wins, and the hook in that env lets the close by.
        let mut old = worker_argv("w", "air", std::path::Path::new("/r/roles.md"), &[]);
        old.extend(["--settings".to_string(), theirs.to_string()]);
        let last = old
            .iter()
            .rposition(|a| a == "--settings")
            .and_then(|i| old.get(i.saturating_add(1)))
            .cloned()
            .unwrap_or_default();
        let last_env = env_of(&last);
        let (code, _) = hook(&last_env)?;
        let red = !last_env.iter().any(|(k, _)| k == "AIR_ENFORCE") && code == 0;

        // GREEN: launch for real against a stub that records argv and env.
        let stub = dir.join("claude-stub");
        let argv_file = dir.join("argv");
        let env_file = dir.join("env");
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\0' \"$@\" > {a}.tmp && mv {a}.tmp {a}\nenv > {e}.tmp && mv {e}.tmp {e}\n",
                a = argv_file.display(),
                e = env_file.display()
            ),
        )
        .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        let socket = format!("air-selftest-{}", new_id());
        // Scrubbed by `air_command`, so the stub can only have the identity variables if the
        // launcher delivered them.
        let out = air_command(&exe, &dir)
            .env("AIR_CLAUDE_BIN", &stub)
            .env("AIR_TMUX_SOCKET", &socket)
            .env_remove("AIR_TMUX_MODE")
            .args(["worker", "w", "--task", "hi", "--", "--settings", theirs])
            .output()
            .map_err(|e| e.to_string())?;
        let mut got = None;
        for _ in 0..6000 {
            if let (Ok(a), Ok(e)) = (
                std::fs::read(&argv_file),
                std::fs::read_to_string(&env_file),
            ) {
                got = Some((a, e));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = Command::new("tmux")
            .args(["-L", &socket, "kill-server"])
            .output();
        if !out.status.success() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        let Some((raw_argv, raw_env)) = got else {
            let _ = std::fs::remove_dir_all(&dir);
            return Err("stub never ran".into());
        };
        let argv: Vec<String> = String::from_utf8_lossy(&raw_argv)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let blobs: Vec<&String> = argv
            .iter()
            .enumerate()
            .filter(|(_, a)| *a == "--settings")
            .filter_map(|(i, _)| argv.get(i.saturating_add(1)))
            .collect();
        let merged = blobs.len() == 1
            && blobs
                .first()
                .and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok())
                .is_some_and(|v| {
                    v.get("remoteControlAtStartup") == Some(&serde_json::Value::Bool(false))
                        && v.pointer("/env/AIR_ENFORCE").and_then(|x| x.as_str()) == Some("1")
                });
        let recorded: Vec<(String, String)> = raw_env
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let delivered = recorded.iter().any(|(k, v)| k == "AIR_ENFORCE" && v == "1")
            && recorded.iter().any(|(k, v)| k == "BEADS_ACTOR" && v == "w");
        let (code, err) = hook(&recorded)?;
        let _ = std::fs::remove_dir_all(&dir);
        let refused = code == 2 && err.contains("air record verify");
        Ok((red, merged && delivered && refused))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "launch: Air's env survives a pass-through --settings and reaches the hook, which refuses a close without green",
        red_fires: red,
        green_passes: green,
    }
}

/// air-fdz: Air creates and fills a worker's worktree, and removes it only when nothing holds
/// it. The blocker the audit found: the adopter's `.worktreeinclude` copies gitignored files a
/// worktree cannot build without (`backend/keys/*.pem` is read by `include_str!` at compile
/// time), and their own note says the error does not reveal why. Red: a naive `git worktree
/// add` gives a worktree whose build fails, which is exactly the fleet a launcher that only
/// ran git would produce. Green: Air's worktree has the listed files (and only those: an
/// unlisted ignored file is not copied, a symlink is skipped), sits on `worktree-<name>`
/// under `.claude/worktrees/<name>`, and its build passes; then removal is refused while the
/// tree carries uncommitted work, naming the file, and removes a clean tree keeping the
/// branch. "A worktree that builds is the test, not a file listing."
fn probe_worktree_is_airs() -> Probe {
    use crate::cmd::worktree::{Holding, branch_for, ensure, holdings, remove};
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |cwd: &Path, args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(cwd)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&dir, &["init", "-q", "-b", "main"])?;
        // The adopter's shape in miniature: two gitignored files the build needs, one it does
        // not, a symlink beside them, and the include file naming the first two.
        std::fs::create_dir_all(dir.join("backend").join("keys")).map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join(".gitignore"),
            ".env\n*.pem\nsecret.txt\n.claude/worktrees/\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join(".worktreeinclude"),
            "# copied into every worktree\nbackend/.env\nbackend/keys/*.pem\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join("build.sh"),
            "#!/bin/sh\ntest -f backend/.env && test -f backend/keys/dev.pem\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("backend").join(".env"), "DATABASE_URL=x\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("backend").join("keys").join("dev.pem"), "KEY\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("backend").join("secret.txt"), "not listed\n")
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        std::os::unix::fs::symlink("dev.pem", dir.join("backend").join("keys").join("link.pem"))
            .map_err(|e| e.to_string())?;
        g(&dir, &["add", "-A"])?;
        g(&dir, &["commit", "-q", "-m", "a"])?;
        let builds = |wt: &Path| -> bool {
            Command::new("sh")
                .arg("build.sh")
                .current_dir(wt)
                .output()
                .is_ok_and(|o| o.status.success())
        };

        // RED: git alone. The tracked build script is there and the build fails.
        let naive = dir.join("naive");
        g(
            &dir,
            &["worktree", "add", "-q", &naive.display().to_string()],
        )?;
        let red = naive.join("build.sh").is_file() && !builds(&naive);

        // GREEN: Air's worktree.
        let made = ensure(&dir, "w")?;
        let wt = made.path.clone();
        let on_branch = g(&wt, &["rev-parse", "--abbrev-ref", "HEAD"])? == branch_for("w");
        let placed = wt == dir.join(".claude").join("worktrees").join("w") && !made.existed;
        let files_right = builds(&wt)
            && !wt.join("backend").join("secret.txt").exists()
            && !wt.join("backend").join("keys").join("link.pem").exists()
            && made.copied.copied.len() == 2
            && made.copied.skipped.iter().any(|s| s.contains("link.pem"));
        // A relaunch finds it and re-copies the current file.
        std::fs::write(dir.join("backend").join(".env"), "DATABASE_URL=y\n")
            .map_err(|e| e.to_string())?;
        let again = ensure(&dir, "w")?;
        let relaunch = again.existed
            && std::fs::read_to_string(wt.join("backend").join(".env")).unwrap_or_default()
                == "DATABASE_URL=y\n";
        // Removal: refused while dirty, naming the file; then removed, branch kept.
        std::fs::write(wt.join("wip.txt"), "unsaved\n").map_err(|e| e.to_string())?;
        let held = holdings(&dir, "w")?;
        let refused = matches!(remove(&dir, "w"), Err(e) if e.contains("wip.txt"))
            && held
                .iter()
                .any(|h| matches!(h, Holding::Dirty(f) if f.iter().any(|x| x.contains("wip.txt"))))
            && wt.is_dir();
        std::fs::remove_file(wt.join("wip.txt")).map_err(|e| e.to_string())?;
        let removed = remove(&dir, "w").is_ok()
            && !wt.exists()
            && g(
                &dir,
                &["rev-parse", "--verify", "-q", "refs/heads/worktree-w"],
            )
            .is_ok();
        let _ = std::fs::remove_dir_all(&dir);
        Ok((
            red,
            on_branch && placed && files_right && relaunch && refused && removed,
        ))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "worktree: Air's worktree carries .worktreeinclude's files and builds; git alone does not; removal refuses uncommitted work and keeps the branch",
        red_fires: red,
        green_passes: green,
    }
}

/// air-bp0: `bd_calls`/`bd_ms` on an event line are that event's own cost. They were the
/// process's running total, which is the same thing in a one-shot command and a different
/// thing in `air mcp`, whose poll thread emits a line every tick for the life of the server:
/// The adopter's 2026-08-30 log summed to 570,989 bd calls while the largest total any process
/// reached was 1,661. Red: the lifetime counter keeps everything and would be restamped on
/// each line. Green: `take` hands each event only what happened since the previous one, and
/// nothing when nothing did.
fn probe_bd_calls_are_per_event() -> Probe {
    use air_bd::stats::{record, snapshot, take};
    let _ = take(); // drain what earlier probes spent, so this window starts empty
    record(5);
    record(7);
    let first = take();
    record(1);
    let second = take();
    let third = take();
    let (_, lifetime) = snapshot();
    Probe {
        name: "events: bd_calls on a line is that event's own count, not the process's running total",
        red_fires: lifetime >= 3 && lifetime > second.1,
        green_passes: first == (12, 2) && second == (1, 1) && third == (0, 0),
    }
}

/// A fake `bd` for the two probes below: logs every argv to `<dir>/bd.log`, answers `list`
/// and `ready` from files, and `show` with a fixed status per id. Instant, and the log is the
/// count of processes, which is the whole cost (`air_bd::stats`).
fn fake_bd_script(dir: &Path) -> Result<std::path::PathBuf, String> {
    let script = dir.join("bd");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nd='{d}'\necho \"$@\" >> \"$d/bd.log\"\ncase \"$1\" in\n  \
             --version) echo 'bd version 1.2.2'; exit 0;;\n  \
             show) shift; out=''; for id in \"$@\"; do case \"$id\" in --*) continue;; esac\n    \
             case \"$id\" in zz-1) s=closed;; zz-2) s=awaiting_review;; *) s=open;; esac\n    \
             out=\"$out${{out:+,}}{{\\\"id\\\":\\\"$id\\\",\\\"status\\\":\\\"$s\\\",\\\"labels\\\":[]}}\"; done\n    \
             printf '%s\\n' \"[$out]\"; exit 0;;\n  \
             list) echo '[]'; exit 0;;\n  \
             ready) if [ -f \"$d/bd.ready\" ]; then cat \"$d/bd.ready\"; else echo '[]'; fi; exit 0;;\n  \
             *) exit 0;;\nesac\n",
            d = dir.display()
        ),
    )
    .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(script)
}

fn bd_log(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("bd.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// air-bp0: `air status` reconciles every open claim bd no longer holds in_progress with ONE
/// `bd show a b c`, not one process per bead. bd's cost is per process (~2 s to open the
/// store) and the query is close to free, so K claims cost K × 2 s before and 2 s after.
/// Red: the old shape, one `show` per id, is K processes for K ids against the same fake bd.
/// Green: a real `air status` over three such claims runs exactly three bd processes (list,
/// one show naming all three, ready), and every claim ends where the per-bead loop put it:
/// the closed bead released as `closed`, the reopened one as `reconciled`, the
/// awaiting_review one kept and marked handed over. Outputs, not only the count.
fn probe_status_reconcile_is_one_show() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let git = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["init", "-q", "-b", "main"])
            .output()
            .map_err(|e| e.to_string())?;
        if !git.status.success() {
            return Err(String::from_utf8_lossy(&git.stderr).to_string());
        }
        let script = fake_bd_script(&dir)?;
        let ids = ["zz-1", "zz-2", "zz-3"];
        // RED: the shape the loop had, one process per bead.
        let mut old = air_bd::BdCli::new(&dir);
        old.bin = script.clone();
        // air-g7e: this probe counts bd PROCESSES, not seconds. At load 145 the default 10 s
        // budget killed a stub before it appended its line, the count came up short, and the
        // probe reported the rule broken 8 runs out of 8. A budget is a ceiling, so a
        // generous one costs nothing when the answer arrives.
        old.timeout = std::time::Duration::from_secs(120);
        for id in ids {
            let _ = air_bd::WorkLedger::show(&old, id);
        }
        let red = bd_log(&dir)
            .iter()
            .filter(|l| l.starts_with("show "))
            .count()
            == ids.len();
        let _ = std::fs::remove_file(dir.join("bd.log"));

        // GREEN: three open claims, none in_progress in bd; one status run.
        let l = Ledger::open_for_repo(&dir).map_err(|e| e.to_string())?;
        for id in ids {
            l.record_claim(id, "w", &[], "t0")
                .map_err(|e| e.to_string())?;
        }
        drop(l);
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let out = air_command(&exe, &dir)
            .env("AIR_BD_BIN", &script)
            // Same reason, for the child: set, this also stops `air status` deriving its own
            // budget from bd's measured median, which is the other way this count goes short.
            .env("AIR_BD_TIMEOUT_MS", "120000")
            .args(["--json", "status"])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        let log = bd_log(&dir);
        let shows: Vec<&String> = log.iter().filter(|l| l.starts_with("show ")).collect();
        let one_show = shows.len() == 1
            && shows
                .first()
                .is_some_and(|s| ids.iter().all(|id| s.split(' ').any(|w| w == *id)));
        let three_processes = log.len() == 3;
        let conn = rusqlite::Connection::open(dir.join(".air").join("ledger.db"))
            .map_err(|e| e.to_string())?;
        let row = |bead: &str| -> Result<(Option<String>, Option<String>), String> {
            conn.query_row(
                "SELECT release_reason, first_handover_at FROM claims WHERE bead=?1",
                rusqlite::params![bead],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())
        };
        let closed = row("zz-1")?;
        let handed = row("zz-2")?;
        let reopened = row("zz-3")?;
        let _ = std::fs::remove_dir_all(&dir);
        let outputs = closed.0.as_deref() == Some("closed")
            && reopened.0.as_deref() == Some("reconciled")
            && handed.0.is_none()
            && handed.1.is_some();
        Ok((red, one_show && three_processes && outputs))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "status: every claim bd no longer holds is looked up in ONE bd show, and each ends where the per-bead loop put it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-80x.4: a red at a batch head is reported by member, in `air record`'s line and in
/// `air status`, and nothing lands on it. A red at a worker's own head (no members) is an
/// ordinary red and is not reported as a batch.
///
/// Red: a red run recorded with members is a red batch whose line names each member, and
/// `air land`'s branch check refuses a branch with no green. Green: a red run with no members
/// and a green run with members are not red batches, and a snapshot without one prints no
/// batch line.
fn probe_red_batch_is_reported_by_member_and_lands_nothing() -> Probe {
    use crate::cmd::batch::{red_batch_line, red_batches_of};
    use crate::cmd::land::{Facts, branch_check};
    use crate::cmd::status::{Snapshot, render_for_probe};
    use air_ledger::landings::Member;

    let run = |sha: &str, exit: i32, members: Vec<Member>| VerifyRun {
        id: new_id(),
        worker: "lane".into(),
        sha: sha.into(),
        kind: Kind::Verify,
        exit_code: exit,
        trigger: "selftest".into(),
        failing_step: None,
        started_at: "t".into(),
        finished_at: "t".into(),
        log_path: None,
        command: None,
        duration_ms: None,
        output_bytes: None,
        dirty: false,
        tree: None,
        members,
    };
    let m = |w: &str, sha: &str| Member {
        worker: w.into(),
        sha: sha.into(),
    };
    let red_batch = run(
        "batch1234",
        2,
        vec![m("alpha", "a1a1a1a1a1"), m("beta", "b2b2b2b2b2")],
    );
    let reds = red_batches_of(std::slice::from_ref(&red_batch));
    let line = reds.first().map(red_batch_line).unwrap_or_default();
    let shown = render_for_probe(&Snapshot {
        red_batch: reds.first().cloned(),
        ..Default::default()
    });
    // Nothing lands on a red: the branch check wants a green at the head, batch or not.
    let refused = branch_check(&Facts {
        worker: "lane",
        branch_exists: true,
        already_in_main: false,
        contains_main: true,
        branch_head: "batch1234",
        green_at: None,
    })
    .is_err();
    let red = reds.len() == 1
        && line.starts_with("batch red at batch123")
        && line.contains("alpha@a1a1a1a1")
        && line.contains("beta@b2b2b2b2")
        && line.contains("nothing lands")
        && shown.contains("batch red at")
        && refused;

    let plain_red = run("own1234567", 2, vec![]);
    let green_batch = run("batch5678", 0, vec![m("alpha", "a1a1a1a1a1")]);
    let none = red_batches_of(&[plain_red, green_batch]);
    let quiet = render_for_probe(&Snapshot::default());
    let green = none.is_empty() && !quiet.contains("batch red");

    Probe {
        name: "record: a red at a batch head is reported by member and lands nothing; a red at a worker's own head is not a batch",
        red_fires: red,
        green_passes: green,
    }
}

/// air-d61: `air doctor` and `air status` say when the install record lags the binary. The adopter's
/// hooks ran 0.2.18 for days on a record that said 0.1.0 / surface 2, and doctor said nothing.
///
/// Red: a temp `.air` whose `installed.json` is older than this binary yields the line, with
/// both versions, the unread notice count, and the fix, in the doctor render and the status
/// render. Green: a record at this binary yields no line, and so does no record at all (told
/// about nothing, which air-w9d deliberately allows).
fn probe_install_lag_is_named() -> Probe {
    use crate::cmd::install::{Installed, SURFACE_VERSION, lag, lag_line, surface_diff};
    use crate::cmd::status::{Snapshot, render_for_probe};

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let write = |rec: &Installed| -> Result<(), String> {
            let s = serde_json::to_string(rec).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("installed.json"), s).map_err(|e| e.to_string())
        };
        // Behind on the surface version alone: the branch the declared mutation neutralises.
        let old = Installed {
            air_version: env!("CARGO_PKG_VERSION").into(),
            surface_version: Some(2),
            surface: vec!["first".into()],
            ..Default::default()
        };
        write(&old)?;
        let unread = surface_diff(&old.surface).len();
        let l = lag(&dir);
        let line = l.as_ref().map(lag_line).unwrap_or_default();
        let shown = render_for_probe(&Snapshot {
            install_lag: l.clone(),
            ..Default::default()
        });
        let surface_lag = l.is_some()
            && line.starts_with(&format!(
                "install record lags the binary: installed {} / surface 2, running {} / surface {SURFACE_VERSION}",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_VERSION")
            ))
            && line.contains(&format!("{unread} notice(s) unread"))
            && line.contains("air install --write")
            && shown.contains("install record lags");
        // Behind on the crate version alone (a record written before surface versions
        // existed carries only that): the other branch.
        write(&Installed {
            air_version: "0.1.0".into(),
            ..Default::default()
        })?;
        let crate_lag = lag(&dir)
            .as_ref()
            .map(lag_line)
            .is_some_and(|s| s.contains("installed 0.1.0 / surface none"));
        let red = surface_lag && crate_lag;

        let current = Installed {
            air_version: env!("CARGO_PKG_VERSION").into(),
            surface_version: Some(SURFACE_VERSION),
            ..Default::default()
        };
        write(&current)?;
        let at_binary = lag(&dir).is_none();
        std::fs::remove_file(dir.join("installed.json")).map_err(|e| e.to_string())?;
        let no_record = lag(&dir).is_none();
        let quiet = !render_for_probe(&Snapshot::default()).contains("install record");
        std::fs::remove_dir_all(&dir).ok();
        Ok((red, at_binary && no_record && quiet))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "doctor: an install record older than the binary is named with both versions, the unread notices and the fix; a current or absent one is not",
        red_fires: red,
        green_passes: green,
    }
}

/// air-80x.1: a verify lane's green at a batch commit closes the bead it covers. Per bead: every
/// commit in `main..HEAD` carrying the bead's trailer is an ancestor of the verified commit C,
/// and C contains main. The adopter's lane's green closed nothing because the gate wanted a
/// green AT the worker's HEAD.
///
/// Red: a batch cut before the worker's last commit is refused, and the refusal names that
/// commit. Green: a batch containing main and every commit of the bead passes the gate with no
/// green at HEAD, the ok line names it, and a green that lacks main never counts.
fn probe_batch_green_closes_the_bead_it_covers() -> Probe {
    use crate::cmd::batch::{BeadCommit, Candidate, cover};

    let commit = |sha: &str| BeadCommit {
        sha: sha.into(),
        subject: format!("work {sha}"),
    };
    let cand = |sha: &str, main: bool, contains: &[bool]| Candidate {
        sha: sha.into(),
        worker: "lane".into(),
        contains_main: main,
        contains: contains.to_vec(),
    };
    let commits = vec![commit("c2after"), commit("c1")];

    // RED: the batch was cut after c1 and before c2after; it must not close the bead, and
    // the refusal must name c2after.
    let early = cover(&[cand("batch1", true, &[false, true])], &commits);
    let mut f = base_facts();
    f.green_at_head = false;
    f.batch_predates = early.predates.as_ref().map(|(sha, w, missing, subject)| {
        format!("the batch's green at {sha} (by {w}) predates your commit {missing} \"{subject}\"")
    });
    let refused = handover_verdict(&f);
    let red = early.covering.is_none()
        && refused.block
        && refused
            .missing
            .iter()
            .any(|m| m.check == "verify-green-at-head" && m.detail.contains("c2after"));

    // GREEN: a later batch contains both commits and main; the gate passes on it alone.
    let late = cover(
        &[
            cand("batch2", true, &[true, true]),
            cand("batch1", true, &[false, true]),
        ],
        &commits,
    );
    let mut g = base_facts();
    g.green_at_head = false;
    g.batch_green = late
        .covering
        .as_ref()
        .map(|(sha, w)| format!("green at {sha} (batch by {w}) contains every commit of zz-1"));
    let passed = handover_verdict(&g);
    // A green that lacks main never covers, whatever it contains.
    let no_main = cover(&[cand("stray", false, &[true, true])], &commits);
    let green = late.covering == Some(("batch2".into(), "lane".into()))
        && passed.pass
        && passed.message.contains("batch by lane")
        && no_main.covering.is_none()
        && no_main.predates.is_none();

    Probe {
        name: "gate: a batch green that contains main and every commit of the bead closes it; one cut before the last commit is refused naming that commit",
        red_fires: red,
        green_passes: green,
    }
}

/// air-bm3 (owner, 2026-08-30): `AskUserQuestion` is denied to workers, and the PreToolUse
/// matcher carries it so an attempt is recorded and the deny is countable. The owner is
/// reached through `air capture`, filed by the coordinator as a bead labelled `owner`.
///
/// Red: the worker argv denies the tool, the coordinator's does not, and the matcher names
/// it. Green: the deny is the bare tool name (the shape `EnterWorktree` uses), and the
/// worker is not denied `air capture`, which is the path that does work.
fn probe_worker_cannot_ask_the_owner_directly() -> Probe {
    use crate::cmd::install::hook_entries;
    use crate::cmd::launch::{coordinator_argv, worker_argv};

    let coord = coordinator_argv("air", Path::new("/r/.air/roles.md"), "--channels", &[], &[]);
    let worker = worker_argv("w", "air", Path::new("/r/.air/roles.md"), &[]);
    let denies = |v: &[String], pat: &str| v.iter().any(|a| a == pat);
    let matcher_counts_it = hook_entries().iter().any(|(event, m)| {
        *event == "PreToolUse" && m.is_some_and(|m| m.contains("AskUserQuestion"))
    });

    let red = denies(&worker, "AskUserQuestion")
        && !denies(&coord, "AskUserQuestion")
        && matcher_counts_it;
    let green = !worker.iter().any(|a| a.contains("AskUserQuestion("))
        && !worker.iter().any(|a| a.contains("air capture"));
    Probe {
        name: "launch: a worker is denied AskUserQuestion and the hook counts the attempt; the coordinator is not, and air capture stays open",
        red_fires: red,
        green_passes: green,
    }
}

/// air-dws: every probe that spawns `air` goes through [`air_command`], which strips the
/// launcher's identity variables, so a probe's verdict does not depend on the shell it runs
/// in. The SubagentStop probe inherited `AIR_ROLE=coordinator` from the coordinator's shell,
/// took the coordinator path, and made `make release` red on main while every worker's
/// worktree was green.
///
/// Red: a source with a raw `Command::new(exe)` beside the helper is counted as one. Green:
/// this file holds exactly one, the helper's own.
fn probe_every_air_spawn_pins_identity() -> Probe {
    let here = include_str!("selftest.rs");
    // Assembled in pieces so this line is not itself a raw spawn to the scan.
    let raw = ["    let out = Command::new(", "&exe).arg(\"hook\");"].concat();
    let with_a_raw_spawn = format!("{here}\n{raw}\n");
    Probe {
        name: "selftest: every probe that spawns air pins its identity through air_command; a raw spawn is caught",
        red_fires: raw_air_spawns(&with_a_raw_spawn) == 2,
        green_passes: raw_air_spawns(here) == 1,
    }
}

/// air-bp0: a subagent stopping is not the worker stopping. The Stop arm marked the session
/// idle and, with no claim held and beads ready, ran the nudge's `bd ready` confirm: one bd
/// process per subagent stop, 241 on the adopter's 2026-08-30, none of them actionable. Red: a
/// Stop in that state does reach bd (the path SubagentStop shared). Green: a SubagentStop in
/// the same state runs no bd process and leaves the session's state as it was.
fn probe_subagent_stop_is_not_a_stop() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<(), String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let wt = dir.join(".claude").join("worktrees").join("w");
        g(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "w",
            &wt.display().to_string(),
        ])?;
        let script = fake_bd_script(&dir)?;
        std::fs::write(
            dir.join("bd.ready"),
            r#"[{"id":"zz-9","status":"open","labels":[]}]"#,
        )
        .map_err(|e| e.to_string())?;
        // The cache write is best-effort and needs `.air/` to exist; opening the ledger
        // creates it, as the first hook would.
        drop(Ledger::open_for_repo(&dir).map_err(|e| e.to_string())?);
        crate::cmd::ready_cache::write(&dir, &["zz-9".to_string()], &crate::cmd::now());
        if crate::cmd::ready_cache::read(&dir).is_none_or(|c| c.ids.is_empty()) {
            return Err("ready cache was not written".into());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let hook = |event: &str, tool: Option<&str>| -> Result<(), String> {
            use std::io::Write;
            let mut input = serde_json::json!({
                "hook_event_name": event,
                "session_id": "air-bp0-probe",
                "cwd": wt.display().to_string(),
            });
            if let (Some(t), Some(obj)) = (tool, input.as_object_mut()) {
                obj.insert("tool_name".into(), serde_json::Value::String(t.to_string()));
            }
            // Pinned identity (air-dws): a worker's worktree, no inherited role.
            let mut child = air_command(&exe, &wt)
                .arg("hook")
                .env("AIR_BD_BIN", &script)
                // air-g7e: the nudge's bd budget is 3 s by default and this hook spawns
                // a shell stub inside it. At load 186 that budget becomes the thing under
                // test's enemy rather than its subject: `confirm` returns None, the nudge
                // stays silent BY DESIGN, and the probe reads a correct silence as the
                // rule failing. The rule is what is under test, so the wait comes out.
                .env("AIR_NUDGE_BD_TIMEOUT_MS", "60000")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(input.to_string().as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            child.wait_with_output().map_err(|e| e.to_string())?;
            Ok(())
        };
        let state = || -> Option<String> {
            rusqlite::Connection::open(dir.join(".air").join("ledger.db"))
                .ok()?
                .query_row(
                    "SELECT state FROM sessions WHERE session_id='air-bp0-probe'",
                    [],
                    |r| r.get(0),
                )
                .ok()
        };
        // RED: a Stop with no claim and a ready cache confirms against bd.
        hook("Stop", None)?;
        let red = bd_log(&dir).iter().any(|l| l.starts_with("ready"));
        let _ = std::fs::remove_file(dir.join("bd.log"));
        // GREEN: mid-turn (a tool just ran), a SubagentStop touches neither bd nor the state.
        hook("PreToolUse", Some("Read"))?;
        let before = state();
        hook("SubagentStop", None)?;
        let after = state();
        let green = bd_log(&dir).is_empty() && before.is_some() && before == after;
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "hook: SubagentStop is not the worker's stop; it runs no bd and marks nothing idle",
        red_fires: red,
        green_passes: green,
    }
}

/// air-12k: no session ever reads `stuck`. The state was written only by
/// `HookEvent::PermissionRequest`, which the fleet's auto mode never sends, so the condition
/// built on it fired zero times in any recorded day (case 3b, air-byw) and was deleted on the
/// owner's ruling with the heartbeat as the failsafe.
///
/// Red: a real `air hook` PermissionRequest leaves the session's state exactly as it was, no
/// attention kind or registry row is named `stuck`, and a session row that somehow holds the
/// state raises nothing. Green: the arm that replaced it is live, so an idle session holding a
/// claim still raises `idle-with-claim`.
fn probe_no_session_reads_stuck() -> Probe {
    use crate::cmd::mechanisms::{Fires, MECHANISMS};
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention, kinds};
    use air_ledger::claims::Claim;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<(), String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let wt = dir.join(".claude").join("worktrees").join("w");
        g(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "w",
            &wt.display().to_string(),
        ])?;
        let script = fake_bd_script(&dir)?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let hook = |event: &str, tool: &str| -> Result<(), String> {
            use std::io::Write;
            let input = serde_json::json!({
                "hook_event_name": event,
                "session_id": "air-12k-probe",
                "cwd": wt.display().to_string(),
                "tool_name": tool,
            });
            let mut child = air_command(&exe, &wt)
                .arg("hook")
                .env("AIR_BD_BIN", &script)
                // air-g7e: the nudge's bd budget is 3 s by default and this hook spawns
                // a shell stub inside it. At load 186 that budget becomes the thing under
                // test's enemy rather than its subject: `confirm` returns None, the nudge
                // stays silent BY DESIGN, and the probe reads a correct silence as the
                // rule failing. The rule is what is under test, so the wait comes out.
                .env("AIR_NUDGE_BD_TIMEOUT_MS", "60000")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(input.to_string().as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            child.wait_with_output().map_err(|e| e.to_string())?;
            Ok(())
        };
        let state = || -> Option<String> {
            rusqlite::Connection::open(dir.join(".air").join("ledger.db"))
                .ok()?
                .query_row(
                    "SELECT state FROM sessions WHERE session_id='air-12k-probe'",
                    [],
                    |r| r.get(0),
                )
                .ok()
        };
        hook("PreToolUse", "Read")?;
        let before = state();
        hook("PermissionRequest", "Bash")?;
        let after = state();
        let _ = std::fs::remove_dir_all(&dir);
        let unchanged = before.is_some() && before == after;
        let unnamed = !kinds::ALL.contains(&"stuck")
            && !MECHANISMS
                .iter()
                .any(|m| m.id == "stuck" || matches!(m.fires, Fires::Condition("stuck")));

        let session = |state: &str| Session {
            state: state.into(),
            changed_at: "2026-01-01T00:00:00Z".into(),
            ..Default::default()
        };
        let held = |worker: &str| Claim {
            bead: "zz-12k".into(),
            worker: worker.into(),
            claimed_at: "2026-01-01T00:00:00Z".into(),
            declared_files: Vec::new(),
            first_handover_at: None,
            last_handover_at: None,
            handover_attempts: 0,
            released_at: None,
            release_reason: None,
        };
        let view = |worker: &str, state: &str| WorkerView {
            worker: worker.into(),
            session: Some(session(state)),
            claims: vec![held(worker)],
            ..Default::default()
        };
        let now = "2026-01-01T02:00:00Z";
        let raised = |state: &str| {
            attention(
                &Snapshot {
                    workers: vec![view("w", state)],
                    ..Default::default()
                },
                now,
                Thresholds::default(),
            )
        };
        let unread = raised("stuck").is_empty();
        let red = unchanged && unnamed && unread;
        let green = raised("idle")
            .iter()
            .any(|a| a.kind == kinds::IDLE_WITH_CLAIM);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "hook: no session ever reads stuck; a permission request changes no state and nothing is named for it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-fzv: the acceptance read's bd budget scales with the id count. It was the client's flat
/// 10 s for the whole id set; the adopter's fourteen-bead batch under the verify lane was
/// refused until they raised `AIR_BD_TIMEOUT_MS` by hand. Measured here 2026-09-06: one
/// `bd show` with fourteen ids takes 21 s, one id 2 s.
///
/// The probe scales the seconds down to milliseconds and keeps the shape: a stub bd that
/// costs a fixed time per id, fourteen ids, and two budgets. Red: under a flat budget the
/// read is refused, and the refusal names the id count, the budget and `AIR_BD_TIMEOUT_MS`.
/// Green: under `base + per_id × ids` the same read answers for every id, and the real
/// budget for fourteen ids is above what bd measured.
fn probe_acceptance_budget_scales_with_ids() -> Probe {
    use crate::cmd::status::{acceptance_budget, acceptance_budget_with, acceptance_with};
    use std::time::Duration;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let script = dir.join("bd");
        // 100 ms per id, slept ONCE for the set (a `sleep` per id costs 200 ms of spawn each
        // on a loaded machine, which is noise, not the shape): fourteen ids cost 1.4 s.
        std::fs::write(
            &script,
            r###"#!/bin/sh
case "$1" in
  show) shift; out=''; n=0
    for id in "$@"; do case "$id" in --*) continue;; esac; n=$((n+1))
      out="$out${out:+,}{\"id\":\"$id\",\"status\":\"open\",\"labels\":[],\"description\":\"## Acceptance Criteria\\n- it lands\"}"
    done
    perl -e "select(undef,undef,undef,$n*0.1)"
    printf '%s\n' "[$out]"; exit 0;;
  *) exit 0;;
esac
"###,
        )
        .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        let ids: Vec<String> = (1..=14).map(|i| format!("zz-{i:02}")).collect();
        let client = |timeout: Duration| air_bd::BdCli {
            bin: script.clone(),
            cwd: dir.clone(),
            timeout,
            label: air_ledger::budgets::BD_ACCEPTANCE,
        };
        // The old shape: one flat budget whatever the count.
        let flat = Duration::from_millis(500);
        let refused = acceptance_with(&client(flat), &ids, false);
        let red = matches!(&refused, Err(m) if m.contains("14 id(s)")
            && m.contains("within a budget of 0.5 s")
            && m.contains("AIR_BD_TIMEOUT_MS"));
        // The new shape, same base, plus an allowance per id. THIRTY times the stub's per-id
        // cost: the probe is about the SHAPE (base + per_id x n), the red half above already
        // proves a flat budget refuses, and the margin is free because a budget is a ceiling
        // and the stub answers in 1.4 s whatever it is set to. Three times was measured at
        // load 53 and lost at load 186; ten times was lost again at load 145 (air-g7e), where
        // this probe failed for the machine's reasons and took four unrelated mutations down
        // with it as "vacuous".
        let scaled = acceptance_budget_with(14, flat, Duration::from_millis(3000));
        let answered = acceptance_with(&client(scaled), &ids, false);
        let all_read = matches!(&answered, Ok(c) if c.len() == 14
            && c.iter().all(|clauses| clauses.len() == 1));
        let real = acceptance_budget(14);
        let covers_measured = real > Duration::from_secs(21) && real > acceptance_budget(1);
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, all_read && covers_measured))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "land: the acceptance read's bd budget grows with the id count, and the refusal names the count, the budget and AIR_BD_TIMEOUT_MS",
        red_fires: red,
        green_passes: green,
    }
}

/// air-1r6: the hook joins `digest_dir` (and every other repo-relative path) to the worktree
/// ROOT, not to the Bash tool's cwd. A persisted `cd crates` made the gate refuse "no digest"
/// for a digest that was there; the adopter's w1 hit it three times on 2026-09-06.
///
/// Red: a real `air hook` gate run on `bd close` from a subdirectory says exactly what the
/// same run from the root says, and neither names a missing digest while the digest exists.
/// Green: with the digest removed, both runs name it missing, so the refusal for a truly
/// absent digest is unchanged.
fn probe_hook_reads_from_the_worktree_root() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<(), String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let wt = dir.join(".claude").join("worktrees").join("w");
        g(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "w",
            &wt.display().to_string(),
        ])?;
        // `.claude/air.json` is read from the MAIN checkout (`handover::air_json`); the digest
        // itself is joined to the worker's own tree.
        let digests = wt.join("docs").join("log.d");
        std::fs::create_dir_all(&digests).map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join(".claude").join("air.json"),
            r#"{"digest_dir": "docs/log.d"}"#,
        )
        .map_err(|e| e.to_string())?;
        let digest = digests.join("2026-09-06-w-zz-1r6.md");
        std::fs::write(&digest, "---\nbead: zz-1r6\n---\n# ours\n").map_err(|e| e.to_string())?;
        let sub = wt.join("crates");
        std::fs::create_dir_all(&sub).map_err(|e| e.to_string())?;
        let script = fake_bd_script(&dir)?;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // The gate's answer, advisory (no AIR_ENFORCE): stdout carries the context, stderr
        // anything the hook says on its own. Both together are "what the gate said".
        let gate = |cwd: &Path| -> Result<String, String> {
            use std::io::Write;
            let input = serde_json::json!({
                "hook_event_name": "PreToolUse",
                "session_id": "air-1r6-probe",
                "cwd": cwd.display().to_string(),
                "tool_name": "Bash",
                "tool_input": {"command": "bd close zz-1r6 --reason done"},
            });
            let mut child = air_command(&exe, cwd)
                .arg("hook")
                .env("AIR_BD_BIN", &script)
                // air-g7e: the nudge's bd budget is 3 s by default and this hook spawns
                // a shell stub inside it. At load 186 that budget becomes the thing under
                // test's enemy rather than its subject: `confirm` returns None, the nudge
                // stays silent BY DESIGN, and the probe reads a correct silence as the
                // rule failing. The rule is what is under test, so the wait comes out.
                .env("AIR_NUDGE_BD_TIMEOUT_MS", "60000")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(input.to_string().as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            Ok(format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        };
        let from_root = gate(&wt)?;
        let from_sub = gate(&sub)?;
        let names_missing = |s: &str| s.contains("no digest in docs/log.d");
        let red = !from_root.is_empty()
            && from_sub == from_root
            && !names_missing(&from_root)
            && !names_missing(&from_sub);
        std::fs::remove_file(&digest).map_err(|e| e.to_string())?;
        let root_missing = gate(&wt)?;
        let sub_missing = gate(&sub)?;
        let green = names_missing(&root_missing) && names_missing(&sub_missing);
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "hook: the gate reads digest_dir from the worktree root, so a close from a subdirectory says what the root says",
        red_fires: red,
        green_passes: green,
    }
}

/// air-09i: a worker stopping with no claim while beads are ready is nudged once. Red: the
/// gate fires on those facts (ready beads, no claim, fresh stop). Green: the block names the
/// beads, then passes once `stop_hook_active` is set (the loop guard) and never for the
/// coordinator.
/// air-ouw: the nudge must never name a bead `air claim` would refuse. Both recorded triggers
/// invalidate a cached list, and neither is a label problem in general:
/// an `owner`-labelled bead (2026-08-22, the migration) and a bead a PEER already claimed
/// (2026-08-22 19:07, the nudge offered alpha the bead beta was holding). `claimable` applied
/// to a LIVE `bd ready` answers both, because bd's ready set is open-and-unblocked, so a
/// claimed bead is already absent and only the label needs filtering. Validating a cached list
/// against labels would have caught the first and missed the second.
///
/// Red: the cache still holds both, which is the behaviour this bead reports. Green: what the
/// nudge actually names, `claimable(live)`, holds neither.
fn probe_nudge_names_only_claimable() -> Probe {
    use crate::cmd::ready_cache::claimable;
    use air_hooks::stop_nudge;

    let issue = |id: &str, status: &str, labels: &[&str]| air_bd::Issue {
        id: id.to_string(),
        status: status.to_string(),
        labels: labels.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    // What the cache was written from, before anything moved.
    let cached: Vec<String> = ["zz-free", "zz-owner", "zz-taken"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // Red: nudging from the cache offers all three, including the two that moved.
    let red = stop_nudge("worker", false, &cached, false)
        .is_some_and(|m| m.contains("zz-owner") && m.contains("zz-taken"));

    // Live bd: the peer's claim left `bd ready` entirely; the owner label did not.
    let live = [
        issue("zz-free", "open", &[]),
        issue("zz-owner", "open", &["owner"]),
    ];
    let confirmed = claimable(&live);
    let green = confirmed == ["zz-free"]
        && stop_nudge("worker", false, &confirmed, false).is_some_and(|m| {
            m.contains("air claim zz-free") && !m.contains("zz-owner") && !m.contains("zz-taken")
        })
        // ...and nothing anywhere admits it might be wrong.
        && !stop_nudge("worker", false, &confirmed, false)
            .is_some_and(|m| m.contains("stale"));
    Probe {
        name: "stop: the nudge names only what air claim would accept (no owner label, no peer's claim)",
        red_fires: red,
        green_passes: green,
    }
}

fn probe_stop_nudge() -> Probe {
    use air_hooks::stop_nudge;
    let ready = vec!["zz-1".to_string()];
    let red = stop_nudge("worker", false, &ready, false).is_some();
    let once =
        stop_nudge("worker", false, &ready, false).is_some_and(|r| r.contains("air claim zz-1"));
    let then_pass = stop_nudge("worker", false, &ready, true).is_none()
        && stop_nudge("coordinator", false, &ready, false).is_none()
        && stop_nudge("worker", true, &ready, false).is_none();
    Probe {
        name: "stop: nudge once when ready beads and no claim",
        red_fires: red,
        green_passes: once && then_pass,
    }
}

/// air-4cr: a verify in flight is a fact `air status` shows and `air land` names.
///
/// The failure: the adopter's coordinator invalidated three workers' verifies in one round by
/// landing under them, with nothing to consult. A full verify is ~420 s there and the landing
/// rate is faster, so their answer was a hand protocol (worker warns, coordinator holds).
///
/// Red: with one run in flight, `air status` prints a line naming the worker and `air land`
/// REFUSES, naming the run, its pid, the fix and the recorded override (air-1bm; it used to
/// warn and land, and the adopter lost 1,199 s of verify to that). Green: with nothing running
/// both are silent, and a run whose process died is not running — the reader prunes it rather
/// than leaving a row nobody can clear.
fn probe_verify_in_flight() -> Probe {
    use crate::cmd::land::in_flight_refusal;
    use crate::cmd::status::{Snapshot, render_for_probe};
    use air_ledger::verify::InFlight;

    let at = "2026-08-29T12:07:00Z";
    let flight = |worker: &str, pid: Option<i64>| InFlight {
        id: format!("id-{worker}"),
        worker: worker.into(),
        sha: "abcdef1234".into(),
        kind: Kind::Verify,
        command: "make verify".into(),
        pid,
        started_at: "2026-08-29T12:00:00Z".into(),
    };
    let snap = |flights: Vec<InFlight>| Snapshot {
        at: at.into(),
        verifies_in_flight: flights,
        ..Default::default()
    };

    let shown = render_for_probe(&snap(vec![flight("alpha", Some(1))]));
    let refused = in_flight_refusal(&[flight("alpha", Some(1))], at);
    let red = shown.contains("verify in flight: alpha")
        // Elapsed in seconds: rounding a just-started run to "0 min" is what makes it look
        // ignorable, and 420 s is the number that decided this bead.
        && shown.contains("420s")
        && refused.as_deref().is_some_and(|m| {
            m.starts_with("refused:")
                && m.contains("alpha")
                && m.contains("420s")
                && m.contains("(pid 1)")
                && m.contains("kill <pid>")
                && m.contains("--despite-inflight")
        });

    // A crashed `air record` leaves a row; the next reader clears it, so nothing accumulates.
    let pruned = (|| -> Result<bool, String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.verify_started(&flight("beta", Some(424_242)))
            .map_err(|e| e.to_string())?;
        let live = l.in_flight_pruned(|_| false).map_err(|e| e.to_string())?;
        let left = l.verifies_in_flight().map_err(|e| e.to_string())?;
        Ok(live.is_empty() && left.is_empty())
    })()
    .unwrap_or(false);

    let green = render_for_probe(&snap(vec![]))
        .lines()
        .all(|x| !x.starts_with("verify in flight"))
        && in_flight_refusal(&[], at).is_none()
        && pruned;
    Probe {
        name: "verify: a run in flight is named by status and refused by land with its pid and the override; nothing running is silent and a dead pid clears",
        red_fires: red,
        green_passes: green,
    }
}

/// air-bxe: a landing has a state, and `air status` holds it.
///
/// The failure: the adopter's coordinator reported a land done three times before the process
/// exited, because the merge commit appears minutes before the verify finishes with the
/// rollback armed. Their workaround was `pgrep`, which produced two defects of its own —
/// `pgrep` printing nothing makes the `ps` after it list every process the user owns, and they
/// read a thirty-line listing as evidence a land was running when it was evidence of the
/// opposite. Separately, a land killed by a closed pipe merged, verified, and wrote no row at
/// all, leaving main green at a sha no landing mentioned.
///
/// Red: an in-flight landing is named, and one whose process is gone says so and names the sha
/// to rewind to. Green: no in-flight landing is silent, and reporting an outcome retires the
/// row without counting as a second attempt.
fn probe_landing_state() -> Probe {
    use crate::cmd::status::{LandingInFlight, Snapshot, landing_in_flight_line, render_for_probe};
    use air_ledger::landings::Landing;

    let at = "2026-08-29T12:02:00Z";
    let row = |result: &str| Landing {
        despite_inflight: vec![],
        members: vec![],
        id: "L1".into(),
        worker: "alpha".into(),
        sha: "branchhead".into(),
        tip_sha: Some("bbbbbbbb99".into()),
        result: result.into(),
        failing_step: None,
        verify_run_id: None,
        attempt_no: 1,
        beads: vec!["air-1".into()],
        open_beads: vec![],
        merge_commit: Some("cccccccc99".into()),
        pid: Some(4242),
        started_at: "2026-08-29T12:00:00Z".into(),
        finished_at: "2026-08-29T12:00:00Z".into(),
    };
    let snap = |flights: Vec<LandingInFlight>| Snapshot {
        at: at.into(),
        landings_in_flight: flights,
        ..Default::default()
    };

    let running = LandingInFlight {
        landing: row("in-flight"),
        alive: Some(true),
    };
    let killed = LandingInFlight {
        landing: row("in-flight"),
        alive: Some(false),
    };
    let shown = render_for_probe(&snap(vec![running.clone()]));
    let killed_line = landing_in_flight_line(&killed, at);
    let red = shown.contains("landing in flight: alpha (air-1) merged at cccccccc")
        && shown.contains("verifying now")
        // "in main" is not "survived": the line has to name the armed rollback target, which
        // is the thing `git merge-base --is-ancestor` cannot tell anyone.
        && shown.contains("rollback armed to bbbbbbbb")
        && killed_line.contains("GONE")
        && killed_line.contains("git reset --hard bbbbbbbb99");

    // The row retires when the outcome is written, and updating it is not a new attempt.
    let lifecycle = (|| -> Result<bool, String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_landing(&row("in-flight"))
            .map_err(|e| e.to_string())?;
        let mid = l.landings_in_flight().map_err(|e| e.to_string())?.len();
        l.record_landing(&row("landed"))
            .map_err(|e| e.to_string())?;
        let after = l.landings_in_flight().map_err(|e| e.to_string())?.len();
        let attempts = l.landing_attempts("alpha").map_err(|e| e.to_string())?;
        Ok(mid == 1 && after == 0 && attempts == 1)
    })()
    .unwrap_or(false);

    let green = render_for_probe(&snap(vec![]))
        .lines()
        .all(|x| !x.starts_with("landing in flight"))
        && lifecycle;
    Probe {
        name: "land: a landing says in-flight from the merge until it reports, a killed one says so with the rewind sha, and reporting retires the row",
        red_fires: red,
        green_passes: green,
    }
}

/// air-d75: Air waits on ten budgets and recorded none of them, so "zero timeouts" in the
/// event log meant nobody was counting. The owner ruled 2026-09-06 "measure all of them from
/// henceforth", and the recording is the deliverable.
///
/// Red: one wait per call, its own budget beside it, and a hit counted only when the caller
/// says the budget was reached. A retry loop reports one sample per wait rather than one per
/// retry, which is what keeps a single 40 ms lock from reading as four waits.
///
/// Green: `take` DRAINS. This is air-bp0 one level up — `bd_ms` restamped a lifetime total on
/// every one of `air mcp`'s poll lines and summed to 570,989 calls against a real maximum of
/// 1,661 — so the same shape gets the same probe before it can happen again.
fn probe_every_wait_is_recorded_once_against_its_own_budget() -> Probe {
    use air_ledger::budgets::{GIT, SQLITE_LOCK, Waits, record, record_progress, take};
    use std::time::Duration;

    let ms = Duration::from_millis;
    let _ = take();
    record(GIT, ms(9), ms(1500), false);
    record(GIT, ms(1500), ms(1500), true);
    // One wait that grows across four handler calls, then a second wait.
    record_progress(SQLITE_LOCK, ms(1), ms(1000), true, false);
    record_progress(SQLITE_LOCK, ms(11), ms(1000), false, false);
    record_progress(SQLITE_LOCK, ms(40), ms(1000), false, false);
    record_progress(SQLITE_LOCK, ms(3), ms(1000), true, false);
    let first = take();

    let got = |name: &str| first.get(name).cloned().unwrap_or_default();
    let red = got(GIT)
        == (Waits {
            budget_ms: 1500,
            n: 2,
            hits: 1,
            ms: vec![9, 1500],
        })
        && got(SQLITE_LOCK)
            == (Waits {
                budget_ms: 1000,
                n: 2,
                hits: 0,
                ms: vec![40, 3],
            });

    // The drain: a second event line carries nothing, and a wait after it carries only itself.
    let empty_after = take().is_empty();
    record(GIT, ms(7), ms(1500), false);
    let second = take();
    let only_the_new_one = second.get(GIT).is_some_and(|w| w.n == 1 && w.ms == vec![7]);

    Probe {
        name: "budgets: one sample per wait against its own budget, and a take drains",
        red_fires: red,
        green_passes: empty_after && only_the_new_one,
    }
}

/// air-d75: every budget name that can reach an event line has a row in `air audit`'s
/// catalogue, so no budget is measured and then never reported.
///
/// This is air-0y9's defect one axis over: a report that silently omits a firing mechanism
/// reads as complete when it is not, which is worse than no report. The catalogue also carries
/// the fail direction, and that is the half a reader cannot supply — a p99 means something
/// different for a budget whose overrun makes the one refusal fail open than for one whose
/// overrun prints an error.
///
/// Red: the catalogue covers exactly the recordable names, one row each. Green: three of them
/// fail OPEN or SILENT, so the distinction is not decorative.
fn probe_every_budget_has_a_catalogue_row_naming_its_fail_direction() -> Probe {
    use crate::cmd::budgets::{CATALOGUE, Fails};

    let rows: std::collections::BTreeSet<&str> = CATALOGUE.iter().map(|b| b.name).collect();
    let names: std::collections::BTreeSet<&str> =
        air_ledger::budgets::NAMES.iter().copied().collect();
    let permitting = CATALOGUE
        .iter()
        .filter(|b| matches!(b.fails, Fails::Open | Fails::Silent))
        .count();

    Probe {
        name: "budgets: every recordable budget has a catalogue row, and the row says which way it fails",
        red_fires: rows == names && rows.len() == CATALOGUE.len(),
        // The three the bead is about: `git` and `sqlite-lock` fail open on a hook path, and
        // the hook's own cap is worse than open — the process is killed and writes nothing.
        green_passes: permitting == 3
            && CATALOGUE
                .iter()
                .any(|b| b.name == air_ledger::budgets::HOOK && b.fails == Fails::Silent),
    }
}

/// air-d75: a real `air hook` invocation records its own wall clock against the cap
/// `air install` writes into `settings.json`, and the `git` calls it made on the way.
///
/// The hook is the budget that matters most and the only one Air cannot observe overrunning:
/// Claude Code kills the process at the cap, and a killed process writes no event line. So the
/// measurement below is of the invocations that finished, and `air audit` pairs it with the
/// unpaired-hook count that is all a killed one leaves.
///
/// Driven through a spawned binary rather than `inner_env`, because what is under test is that
/// the number reaches the file: an in-process call would pass with the append broken.
///
/// Red: the event line carries `budgets.hook` with one wait, the installed cap as its budget,
/// and no hit. Green: it also carries the `git` calls the hook made, so the 1.5 s budget on
/// every hook path is visible rather than assumed.
fn probe_a_hook_records_its_own_wall_clock() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<(), String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let input = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "air-d75-probe",
            "cwd": dir.display().to_string(),
            "tool_name": "Read",
        });
        let mut child = air_command(&exe, &dir)
            .arg("hook")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(input.to_string().as_bytes())
                .map_err(|e| e.to_string())?;
        }
        child.wait_with_output().map_err(|e| e.to_string())?;

        let events = dir.join(".air").join("events");
        let mut lines: Vec<Value> = Vec::new();
        for entry in std::fs::read_dir(&events).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            lines.extend(text.lines().filter_map(|l| serde_json::from_str(l).ok()));
        }
        let _ = std::fs::remove_dir_all(&dir);

        let cap = crate::cmd::install::HOOK_TIMEOUT_SECS.saturating_mul(1000);
        let Some(line) = lines
            .iter()
            .find(|v| v.get("command").and_then(Value::as_str) == Some("hook.PreToolUse"))
        else {
            return Ok((false, false));
        };
        let at = |p: &str| line.pointer(p).and_then(Value::as_u64);
        let recorded = at("/budgets/hook/n") == Some(1)
            && at("/budgets/hook/budget_ms") == Some(cap)
            && at("/budgets/hook/hits") == Some(0)
            && line
                .pointer("/budgets/hook/ms")
                .and_then(Value::as_array)
                .is_some_and(|a| a.len() == 1);
        // The hook resolves its worktree root through `git.rs`, so the 1.5 s budget on the
        // hook path is exercised by the same invocation.
        let git_too = at("/budgets/git/n").is_some_and(|n| n > 0)
            && at("/budgets/git/budget_ms") == Some(1500);
        Ok((recorded, git_too))
    })()
    .unwrap_or_else(blocked);

    Probe {
        name: "budgets: a real hook invocation records its own wall clock and the git calls it made",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-d75: what a hook killed at its cap leaves behind, since it cannot leave a measurement.
///
/// A tool call that got its PreToolUse should reach exactly one of PostToolUse,
/// PermissionDenied or PostToolUseFailure. When it reaches none, a hook did not run. Hook
/// payloads carry no tool-call id, so the pairing is per (session, tool) and the components
/// are printed with the difference: this is an inference over counts, and an unexplained
/// "2 hooks lost" is the derived-reads-like-observed failure air-21c records.
///
/// The pairable set is DERIVED from `install::hook_entries`, not written down here: the
/// matchers have changed as tools were added to be counted (`SendMessage`, `AskUserQuestion`),
/// and a hardcoded list would go on reporting confidently about tools Air no longer hooks.
///
/// Red: a Pre with no Post is counted; one answered by a Post, a denial or a failure is not.
/// Green: a Post with no Pre is counted separately rather than netted away, because that is
/// the other direction of the same loss — and a tool outside the pairable set is ignored.
fn probe_an_unpaired_hook_is_counted_from_the_installed_matchers() -> Probe {
    use crate::cmd::budgets::{budgets_of, paired_tools};

    let paired = paired_tools();
    let line = |command: &str, session: &str, tool: &str| {
        format!(
            r#"{{"at":"2026-09-06T01:00:00Z","worker":"w","command":"hook.{command}","decision":"x","inputs":{{"session_id":"{session}","tool":"{tool}"}}}}"#
        )
    };
    let day = [
        // s1/Edit: two Pre, one Post -> one unpaired.
        line("PreToolUse", "s1", "Edit"),
        line("PreToolUse", "s1", "Edit"),
        line("PostToolUse", "s1", "Edit"),
        // s2/Bash: a Pre answered by a denial, and one by a failure. Neither is a loss.
        line("PreToolUse", "s2", "Bash"),
        line("PermissionDenied", "s2", "Bash"),
        line("PreToolUse", "s2", "Bash"),
        line("PostToolUseFailure", "s2", "Bash"),
        // s3/Write: a Post with no Pre -> the other direction.
        line("PostToolUse", "s3", "Write"),
        // A tool the PostToolUse matcher does not cover cannot be paired and is ignored.
        line("PreToolUse", "s4", "SendMessage"),
    ]
    .join("\n");
    let h = budgets_of(&[("2026-09-06".to_string(), day)], "2026-09-06").hooks;

    Probe {
        name: "budgets: an unpaired hook is counted from the installed matchers, in both directions",
        red_fires: h.pre_unmatched == 1 && h.pre == 4 && h.denied == 1 && h.failed == 1,
        green_passes: h.post_unmatched == 1
            && paired.contains(&"Edit".to_string())
            && !paired.contains(&"SendMessage".to_string()),
    }
}

/// air-mir: releases are cut per round, not per notice-bearing landing. Owner ruled 2026-09-06
/// after nineteen releases in one day, five release-row number collisions between lanes
/// re-numbered by coordinator message, and a tag/verify/install cycle of several minutes on
/// main for every landing that carried a notice.
///
/// The invariant did not move: a surface notice never ships without a `RELEASES` row, so
/// `Installed` never claims a version it cannot identify (air-w9d). WHEN it is asked did — at
/// `make release`, not at every `make verify`.
///
/// Red: a tree with an unreleased notice passes verify and is refused by the release check,
/// and the refusal names the count and the row to append rather than saying no. Green: the
/// directions that must still fail do — a notice REMOVED or a row edited to say less is
/// refused at verify time, and a crate version disagreeing with the last row is refused at
/// release time even with the count right.
///
/// Every number here is read from the real last row, so a released count cannot be copied into
/// the fixture and rot beside it (air-jc0).
fn probe_a_notice_waits_for_the_round_and_the_release_refuses() -> Probe {
    use crate::cmd::install::{RELEASES, release_check, verify_rows_ok};

    let (version, surface, count) = RELEASES.last().copied().unwrap_or(("0.0.0", 0, 0));
    let one_more = count.saturating_add(1);

    let refusal = release_check(version, one_more);
    let names_the_row = refusal.as_ref().err().is_some_and(|m| {
        m.contains(&one_more.to_string())
            && m.contains(&surface.saturating_add(1).to_string())
            && m.contains("append")
    });

    Probe {
        name: "release: a lane's notice passes verify and waits for the round; the release check refuses it, naming the row",
        red_fires: verify_rows_ok(count, one_more) && names_the_row,
        green_passes: !verify_rows_ok(count, count.saturating_sub(1))
            && release_check(version, count).is_ok()
            && release_check(&format!("{version}-not"), count).is_err(),
    }
}

/// air-8gj: the harness's `--worktree` isolation is off, and one PreToolUse check replaces it.
///
/// The flag was removed on evidence, not preference: in the adopter's record it stopped no
/// observed write to the main checkout and cost 455 refusals in five days, 388 of them (88%)
/// with no git token in the command, plus a native build refused with no prompt and permission
/// prompts nobody could answer unattended
/// (`private/notes/2026-09-06-answers-worktree-and-verify.md`, owner ruling 2026-09-06).
/// The one gap it did close and nothing else did is a hand-written `../../main/<path>` in a
/// file tool. That is this check, and nothing wider: a Bash `cd ../..` is out of scope on
/// purpose, because the harness never caught it either.
///
/// Driven through a spawned `air hook` against a real repo and a real worktree, not through
/// `fence::denial`: the unit tests already cover the path arithmetic, and what is under test
/// here is that the hook actually BLOCKS — the wiring, the role gate, and the exit code the
/// harness reads.
///
/// Red: the refusals fire. A worker's Edit to a path outside its worktree is blocked, so is a
/// `..` climb out of it, and the refusal names both the path and the worktree. Green: the
/// non-refusals do not. The same worker editing inside is allowed, and the coordinator in the
/// main checkout is never fenced — its checkout IS the root it would be measured against, and
/// the one session whose job is to edit main must not be stopped from doing it.
fn probe_an_edit_outside_the_worktree_is_denied() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<(), String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let wt = dir.join(".claude").join("worktrees").join("w");
        g(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "w",
            &wt.display().to_string(),
        ])?;
        std::fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(wt.join("src")).map_err(|e| e.to_string())?;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // (exit code, stderr) of one PreToolUse Edit from `cwd` at `path`.
        let edit = |cwd: &Path, path: &Path| -> Result<(i32, String), String> {
            use std::io::Write;
            let input = serde_json::json!({
                "hook_event_name": "PreToolUse",
                "session_id": "air-8gj-probe",
                "cwd": cwd.display().to_string(),
                "tool_name": "Edit",
                "tool_input": {"file_path": path.display().to_string()},
            });
            let mut child = air_command(&exe, cwd)
                .arg("hook")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(input.to_string().as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            Ok((
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).to_string(),
            ))
        };

        let outside = dir.join("src").join("a.rs");
        let (out_code, out_err) = edit(&wt, &outside)?;
        let (in_code, _) = edit(&wt, &wt.join("src").join("a.rs"))?;
        // The gap the harness's isolation did close: a hand-written climb out of the worktree.
        let (climb_code, _) = edit(
            &wt,
            &wt.join("..").join("..").join("..").join("src").join("a.rs"),
        )?;
        // The coordinator, in the main checkout, editing the same file the worker was refused.
        let (main_code, _) = edit(&dir, &outside)?;
        let _ = std::fs::remove_dir_all(&dir);

        let named = out_err.contains(&outside.display().to_string())
            && out_err.contains(&wt.display().to_string());
        Ok((
            out_code == 2 && climb_code == 2 && named,
            in_code == 0 && main_code == 0,
        ))
    })()
    .unwrap_or_else(blocked);

    Probe {
        name: "hook: a worker's edit outside its worktree is denied naming the path; inside is allowed and the coordinator in main is never fenced",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-g5o: Metis reaches the coordinator's session and no worker's.
///
/// The owner asked whether making the planning rule programmatic is "what metis does
/// basically". It is not: Metis enforces forward-only phases on its own documents and does not
/// enforce that anyone plans in it (`docs/research/metis-deep-dive.md` §4-5). The harness has
/// no per-ROLE MCP configuration either — a `.mcp.json` in the repo reaches every session,
/// workers included — so the attach is Air's, per role, per launch.
///
/// Red: the coordinator's argv carries `--mcp-config` with metis's own server declaration, and
/// `--plugin-dir` when the repo declared a directory that exists. Green: a worker's argv
/// carries neither, ever, and a declared plugin directory that is NOT a directory is dropped
/// rather than passed — `--plugin-dir` at a path that does not exist loads nothing and says
/// nothing, which is the one direction this must not fail in.
fn probe_metis_is_the_coordinators_and_never_a_workers() -> Probe {
    use crate::cmd::launch::{coordinator_argv, worker_argv};
    use crate::cmd::metis::{Config, MCP_CONFIG, argv, attach, plugin_dir_for};

    let roles = Path::new("/r/.air/coordinator.md");
    let dir = std::env::temp_dir();
    let real = dir.to_string_lossy().to_string();
    let (resolved, _) = plugin_dir_for(&Config {
        on: true,
        plugin_dir: Some(real.clone()),
    });
    let on = coordinator_argv(
        "air",
        roles,
        "--channels",
        &argv(true, resolved.as_deref()),
        &[],
    );
    let pair = |v: &[String], flag: &str| -> Option<String> {
        v.iter()
            .position(|a| a == flag)
            .and_then(|i| v.get(i.saturating_add(1)).cloned())
    };
    let red = pair(&on, "--mcp-config").as_deref() == Some(MCP_CONFIG)
        && pair(&on, "--plugin-dir") == Some(real)
        && MCP_CONFIG.contains("\"metis\"");
    // A server that cannot start is not attached at all: nothing is passed and one line says
    // why. Handing the harness a command that is not there buys a failed server and no more.
    let (absent_argv, absent_notes) = attach(
        &Config {
            on: true,
            plugin_dir: None,
        },
        false,
    );
    let absent = absent_argv.is_empty()
        && absent_notes
            .first()
            .is_some_and(|n| n.contains("not on PATH"));

    // A worker: the same repo, the same config, and none of it.
    let w = worker_argv("w1", "air", Path::new("/r/.air/roles.md"), &[]);
    let worker_clean = !w.iter().any(|a| a == "--mcp-config" || a == "--plugin-dir");
    // Off, and a declared directory that is not one.
    let off = coordinator_argv("air", roles, "--channels", &argv(false, Some("/x")), &[]);
    let (missing, note) = plugin_dir_for(&Config {
        on: true,
        plugin_dir: Some("/nonexistent-zz/plugins/metis".into()),
    });
    let unusable = coordinator_argv(
        "air",
        roles,
        "--channels",
        &argv(true, missing.as_deref()),
        &[],
    );

    Probe {
        name: "launch: metis is attached to the coordinator and to no worker; a plugin dir that is not a directory is dropped, not passed",
        red_fires: red,
        green_passes: worker_clean
            && absent
            && !off.iter().any(|a| a == "--mcp-config")
            && pair(&unusable, "--mcp-config").as_deref() == Some(MCP_CONFIG)
            && !unusable.iter().any(|a| a == "--plugin-dir")
            && note.is_some_and(|n| n.contains("not a directory")),
    }
}

/// air-g5o: which initiative a bead came from is a DECLARED field, and the number Air prints
/// about it refuses nothing.
///
/// The alternative was to look for an initiative code anywhere in the description. That reads
/// a fact out of prose somebody wrote freely (the `anti-brittleness` skill), and it fails
/// toward counting a bead as compliant because its text happened to mention one — the
/// permitting direction.
///
/// Red: a bead with an `initiative: <CODE>` line declares one; a bead that merely mentions an
/// initiative in a sentence does not, and neither does the key with nothing after it. Green:
/// `air status` prints the count with its denominator and the words "not a gate", and says
/// nothing at all when every bead declares one.
fn probe_an_initiative_is_declared_and_counted_without_a_gate() -> Probe {
    use crate::cmd::metis::{initiative_of, without_initiative};
    use crate::cmd::status::{Snapshot, render_for_probe};

    let declared =
        initiative_of("air-g5o does the thing\ninitiative: PLAT-3\n") == Some("PLAT-3".into());
    let mention = initiative_of("this is part of the PLAT-3 initiative").is_none();
    let empty = initiative_of("initiative:  ").is_none();
    let sentence = initiative_of("initiative: the one we agreed on").is_none();

    let issue = |id: &str, ty: &str, desc: &str| air_bd::Issue {
        id: id.into(),
        issue_type: ty.into(),
        description: desc.into(),
        ..Default::default()
    };
    let pool = vec![
        issue("a", "task", "initiative: PLAT-1"),
        issue("b", "task", "nothing declared"),
        issue("c", "epic", "nothing declared, and an epic is a container"),
    ];
    let counted = without_initiative(&pool) == (1, 2);

    let shown = Snapshot {
        without_initiative: Some((1, 2)),
        ..Default::default()
    };
    let text = render_for_probe(&shown);
    let says = text.contains("beads without initiative: 1 of 2") && text.contains("not a gate");

    let none = Snapshot {
        without_initiative: Some((0, 9)),
        ..Default::default()
    };
    let quiet = !render_for_probe(&none).contains("beads without initiative");

    Probe {
        name: "status: an initiative is a declared line, not a mention, and the count that reads it is not a gate",
        red_fires: declared && mention && empty && sentence && counted,
        green_passes: says && quiet,
    }
}

/// air-bpj: no tracked file names an adopter, and the check that says so reads the names from
/// a file that is itself private.
///
/// Owner ruling 2026-09-06: the Air project is separate from the adopter's, so nothing that
/// names them is public. A rule in CLAUDE.md would fail toward PUBLISHING — one forgotten line
/// in a digest and the name is in somebody's clone, which is the one direction that cannot be
/// undone. So it is a check in `make verify`.
///
/// Two things had to be true at once, and they pull against each other: the check must know the
/// names, and the names must not be in the binary or in any tracked file. It reads
/// `private/adopters.md`, which `.gitignore` covers.
///
/// Red: a tracked line naming an adopter is found, case-insensitively (the sweep's own miss was
/// an upper-case table row, which a case-sensitive grep let through), and the refusal names
/// the file and the line. Green: the two ways this must NOT fire — a clean tree passes, and a
/// clone with no `private/adopters.md` SKIPS rather than failing, which is the open-source
/// contributor's case. And the names come from declared `name:` lines only, so the file's own
/// prose and paths do not make it refuse itself.
fn probe_no_tracked_file_names_an_adopter() -> Probe {
    use crate::cmd::privacy::{leaks, names, refusal};

    let md = "# Adopters\n\n    name: acme\n    prefix: ac\n    checkout: ~/projects/acme\n\n\
              acme is called \"an adopter\" in tracked text.\n";
    let n = names(md);
    let files = vec![
        (
            "docs/digests/one.md".to_string(),
            "a clean line\nthe row said ACME-KEEPS\n".to_string(),
        ),
        (
            "crates/cli/src/cmd/land.rs".to_string(),
            "// an adopter, 2026-08-31: 455 refusals in five days (see air-8gj)\n".to_string(),
        ),
    ];
    let found = leaks(&n, &files);
    let msg = refusal(&found).unwrap_or_default();
    let red = n == vec!["acme".to_string()]
        && found.len() == 1
        && found.first().is_some_and(|l| l.line == 2)
        && msg.contains("docs/digests/one.md:2");

    // The incident line keeps its date, its count and its air- bead and is not a leak.
    let clean = leaks(&n, files.get(1..).unwrap_or_default()).is_empty();
    // No list: nothing to check, and nothing refused.
    let no_list = names("").is_empty() && leaks(&[], &files).is_empty();
    // The mapping file's own prose and paths are not names, or the check refuses the file it
    // reads.
    let declared_only = names("acme is the adopter\n    checkout: /acme\n").is_empty();

    Probe {
        name: "privacy: a tracked line naming an adopter is refused with its file and line; a clean tree and a clone with no list are not",
        red_fires: red,
        green_passes: clean && no_list && declared_only,
    }
}

/// air-g7e: every declared mutation's anchor still occurs exactly once in the file it names.
///
/// **Two mutations had been BROKEN for days and the suite said nothing.** air-80x.1 rewrote
/// the gate's green branch and air-bm3 widened the PreToolUse matcher; both `from` anchors
/// stopped matching, so `gate: verify-green-at-head` and the SendMessage traffic probe kept
/// printing PASS with no evidence behind them at all. A probe whose mutation cannot be applied
/// is a probe nobody has seen fail, which is precisely what air-682 says is not evidence.
///
/// The only thing that noticed was `air selftest --prove`, and it costs 31 minutes because it
/// rebuilds the binary once per mutation — so it is run at the end of a round, if at all, and
/// both anchors died in between. This costs milliseconds: it reads each file once and counts a
/// substring. No build, no spawn, no ledger.
///
/// Zero occurrences is a dead anchor. More than one is worse than dead: `--prove` would refuse
/// it as ambiguous, and an ambiguous anchor is the wrong-path trap air-682 names, where the
/// mutation lands somewhere other than the branch the probe is about.
///
/// Red: a table whose anchor is missing, and one whose anchor is ambiguous, are both named
/// with their file. Green: every anchor in the REAL table resolves exactly once today.
fn probe_every_declared_mutation_still_anchors() -> Probe {
    // Pure over a reader, so the red half needs no scratch files and the green half reads the
    // real tree.
    fn stale(muts: &[(&str, Mutation)], read: impl Fn(&str) -> Option<String>) -> Vec<String> {
        let mut out = Vec::new();
        for (name, m) in muts {
            let Some(text) = read(m.file) else {
                out.push(format!("{}: cannot read {}", name, m.file));
                continue;
            };
            match text.matches(m.from).count() {
                1 => {}
                0 => out.push(format!("{}: anchor gone from {}", name, m.file)),
                n => out.push(format!("{}: anchor occurs {n} times in {}", name, m.file)),
            }
        }
        out
    }

    let missing = [(
        "made up",
        Mutation {
            file: "crates/cli/src/cmd/install.rs",
            from: "this text is in no file",
            to: "x",
            also_red: &[],
        },
    )];
    let ambiguous = [(
        "made up",
        Mutation {
            file: "crates/cli/src/cmd/install.rs",
            from: "twice",
            to: "x",
            also_red: &[],
        },
    )];
    let fixture = |_: &str| Some("twice and twice again".to_string());
    let red = stale(&missing, |_| Some("nothing like it".to_string()))
        .first()
        .is_some_and(|s| s.contains("anchor gone from crates/cli/src/cmd/install.rs"))
        && stale(&ambiguous, fixture)
            .first()
            .is_some_and(|s| s.contains("occurs 2 times"));

    // Under `--prove` one anchor is deliberately absent — the mutation in flight — so the
    // question this probe asks has no answer then. It stands down rather than reporting a
    // dead anchor that is a live mutation, which would mark every mutation vacuous.
    if std::env::var_os("AIR_SELFTEST_PROVING").is_some() {
        return Probe {
            name: "selftest: every declared mutation still anchors exactly once in the file it names",
            red_fires: red,
            green_passes: true,
        };
    }

    // The real table against the real tree.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let live = stale(MUTATIONS, |f| std::fs::read_to_string(root.join(f)).ok());
    if !live.is_empty() {
        eprintln!("air selftest: dead mutation anchors:");
        for l in &live {
            eprintln!("  {l}");
        }
    }
    // The other half of the registry's health — that every key names a probe that exists — is
    // deliberately NOT checked here. This probe is itself in `all_probes()`, so asking that
    // question would call `all_probes()` from inside it and recurse forever (seen, on the
    // first run of this probe). `--prove` already treats an orphan key as a hard failure, and
    // an orphan is loud there in a way a dead anchor was not.

    Probe {
        name: "selftest: every declared mutation still anchors exactly once in the file it names",
        red_fires: red,
        green_passes: live.is_empty(),
    }
}
