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
//! green. A copied number passes the first and fails the second, which is adopter's ad-m8v1.
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

/// air-682: the edit that neutralises the rule a probe names, declared next to the probe so it
/// can be RUN. adopter's standard, adopted over ours by owner ruling: a probe is evidence only
/// once it has been seen failing with its rule neutralised, and the evidence is a revert, not an
/// intention. `air selftest` claiming "a probe that matches nothing prints red" is weaker,
/// because a vacuous probe also prints red for reasons of its own.
///
/// Three ways a revert demonstration misleads, all three of which `prove` reports separately:
///
/// 1. **A mutant that does not build.** adopter's first run scored 15 of 15 red; two were a
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
        "gate: verify-green-at-head",
        Mutation {
            file: "crates/hooks/src/gate.rs",
            from: "if !f.green_at_head {",
            to: "if false {",
            // The enforced-gate and close-with-proof probes drive the same refusal end to end.
            also_red: &[
                "gate: AIR_ENFORCE=1 denies bd update -s awaiting_review without green at HEAD (names the fix); allows with green",
                "gate: two closes on one unchanged HEAD cost one verify; a commit demands a new one and clears",
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
        "green: a landing reads green from its tree only where the repo declares verify_key tree; an unverified tree never does",
        Mutation {
            // Let a tree green count under the default key. That is the silent upgrade
            // air-7wf refused to ship: adopter's citation gate would have started passing
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
                "launch: --task reaches claude as the prompt by file; the task text is not in argv",
            ],
        },
    ),
    (
        "launch: --task reaches claude as the prompt by file; the task text is not in argv",
        Mutation {
            // Put the task back into argv, which is the shape that killed adopter's workers
            // (air-er0). The file is still written; only the prompt regresses.
            file: "crates/cli/src/cmd/launch.rs",
            from: "Ok(p) => Some(task_prompt(&p)),",
            to: "Ok(_p) => Some(t.to_string()),",
            also_red: &[],
        },
    ),
    (
        "gate: claim required for the named bead",
        Mutation {
            file: "crates/hooks/src/gate.rs",
            from: "if !f.bead_claimed_by_worker {",
            to: "if false {",
            also_red: &[],
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
            // Nothing is ever new, which is the pre-air-6g1 world: install reports success and
            // an adopting repo learns nothing. The green half legitimately still passes (an
            // empty diff for a current repo is what it asserts), so only the red half falls —
            // which is the point of naming one branch.
            file: "crates/cli/src/cmd/install.rs",
            from: "!known.iter().any(|k| k == c.id)",
            to: "false",
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
            from: "Some(\"Edit|Write|MultiEdit|Bash|SendMessage\")",
            to: "Some(\"Edit|Write|MultiEdit|Bash\")",
            also_red: &[],
        },
    ),
    (
        "verify: a run in flight is named by status and warned about by land; nothing running is silent and a dead pid clears",
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
/// probe either way, and reporting it as evidence is adopter's failure mode 1.
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
/// and restores with `git checkout --` after each mutation whatever the outcome.
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
        restore(repo, m.file);
        rows.push(ProofRow { probe, proof });
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

fn restore(repo: &Path, file: &str) {
    let _ = Command::new("git")
        .args(["checkout", "--", file])
        .current_dir(repo)
        .output();
}

/// air-7q5: starting a session must not start work. The owner drew the line at launch time, and
/// the mechanism that holds it is that a worker launched with no `--task` gets NO PROMPT: the
/// roles prose reaches it through `--append-system-prompt-file`, which is context rather than a
/// turn, so an untriggered session never runs. Red: with a task, the task is the prompt and the
/// session is triggered. Green: with no task, argv opens on a flag and carries no positional at
/// all, so there is nothing for claude to answer.
fn probe_no_task_no_prompt() -> Probe {
    use crate::cmd::launch::{task_is_prompt, worker_argv_tmux};
    let base = vec![
        "--append-system-prompt-file".to_string(),
        "/r/.air/roles.md".to_string(),
        "--disallowed-tools".to_string(),
        "Bash(git push *)".to_string(),
    ];
    let task = "work air-1";
    let with = worker_argv_tmux(base.clone(), false, None, Some(task));
    let without = worker_argv_tmux(base.clone(), false, None, None);
    // A blank task is not a task: it must not become an empty prompt either.
    let blank = worker_argv_tmux(base.clone(), false, None, Some("   "));
    Probe {
        name: "launch: a task is the prompt; no task means no prompt, so an untriggered worker never runs",
        red_fires: with.first().is_some_and(|a| a == task) && task_is_prompt(&with, task),
        green_passes: without == base
            && blank == base
            && without.first().is_some_and(|a| a.starts_with('-')),
    }
}

/// air-uae: two lease stores that disagree deny work while reporting success. adopter's
/// `make lease-take` wrote Air's ledger and their guard read `ad-leases/`, so `make api` was
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
        probe_gate_names_the_landing_that_moved_main(),
        probe_handover_ok_names_the_main_it_checked(),
        probe_handover_matcher(),
        probe_ledger_roundtrip(),
        probe_green_follows_the_tree_only_where_declared(),
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
        probe_lease_defect_reaches_the_waiter(),
        probe_yesterdays_repo_is_told_and_a_current_one_is_not(),
        probe_install_goes_forward_only(),
        probe_enforced_gate(),
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
    let (red, green) = res.unwrap_or((false, false));
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
    let (red, green) = res.unwrap_or((false, false));
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
    let (red, green) = res.unwrap_or((false, false));
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

    let coord = coordinator_argv("air", Path::new("/r/.air/roles.md"), "--channels", &[]);
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
/// Red, the bug that made `air land` unusable in adopter: the old narrowing required a claim
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
        l.record_claim("fd-1", "alpha", &[], "2026-08-22T10:00:00Z")
            .map_err(|e| e.to_string())?;
        let kept = crate::cmd::status::attributable_for_test(&l, &["fd-1".to_string()], "alpha")?;
        // ...and a bead already landed is dropped, which is the bound that replaced the time.
        let dropped =
            crate::cmd::status::attributable_for_test(&l, &["ad-other".to_string()], "alpha")?;
        Ok(kept == ["fd-1"] && dropped.is_empty())
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
        l.record_claim("fd-1", "probe", &[], "t0")
            .map_err(|e| e.to_string())?;
        green_at(&head)?;
        let first = passes("fd-1");

        // Bead two, finished without moving HEAD (a docs bead already satisfied, a no-op fix).
        // ONE verify run exists in total, and this close must still pass: a green at a commit
        // that has not moved is still a green.
        l.record_claim("fd-2", "probe", &[], "t1")
            .map_err(|e| e.to_string())?;
        let second_free = passes("fd-2");
        let runs: i64 = l
            .conn()
            .query_row("SELECT count(*) FROM verify_runs", [], |r| r.get(0))
            .unwrap_or(-1);

        // Bead three, with a commit: HEAD moved, so the gate demands a verify there.
        g(&["commit", "-q", "--allow-empty", "-m", "b"])?;
        l.record_claim("fd-3", "probe", &[], "t2")
            .map_err(|e| e.to_string())?;
        let refused_after_commit = !passes("fd-3");
        // ...and recording one at the new HEAD clears it. No stall.
        let moved = g(&["rev-parse", "HEAD"])?;
        green_at(&moved)?;
        let cleared = passes("fd-3");

        Ok((
            refused_after_commit,
            first && second_free && runs == 1 && cleared,
        ))
    })()
    .unwrap_or((false, false));
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
        "fd-2",
        vec!["docs/rules/writing.md names the rule.".into()],
        &ev,
    );
    // A clause Air simply cannot read. Not a defect, and not the wrong-close signal.
    let unreadable = judge_clauses(
        "fd-3",
        vec!["The owner rules on the counter-argument.".into()],
        &ev,
    );
    let discharged = judge_clauses(
        "fd-1",
        vec![
            "Verify recorded green at HEAD.".into(),
            "docs/rules/roles.md names the rule.".into(),
        ],
        &ev,
    );

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_landing(&Landing {
            id: new_id(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: "landed-refuted".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 1,
            beads: vec!["fd-1".into(), "fd-2".into()],
            open_beads: vec![OpenBead {
                bead: "fd-2".into(),
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
                .is_some_and(|o| o.bead == "fd-2" && o.why.contains("docs/rules/writing.md"));
        // air-dlw: the claim's lifetime must NOT decide this. Under close-with-proof the
        // worker closes at once and the reconcile releases the claim on the next tick, so a
        // report keyed on the claim could never fire. Claim it, release it as the reconcile
        // does, and the report has to survive both.
        l.record_claim("fd-2", "alpha", &[], "t2")
            .map_err(|e| e.to_string())?;
        l.release_claims_on(&["fd-2".to_string()], "closed", "t3")
            .map_err(|e| e.to_string())?;
        let survives_the_claim = l.landed_open().map_err(|e| e.to_string())?.len() == 1;

        // It clears when a LATER landing of the same bead stops refuting it.
        l.record_landing(&Landing {
            id: new_id(),
            worker: "alpha".into(),
            sha: "ddd".into(),
            tip_sha: Some("ccc".into()),
            result: "landed".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 2,
            beads: vec!["fd-2".into()],
            open_beads: vec![OpenBead {
                bead: "fd-2".into(),
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
    .unwrap_or((false, false));
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
    let five = ["ad-7p85", "ad-epo9", "ad-fsxg", "ad-lqhf", "ad-xeq3"];

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        // The live case: a refusal carrying five beads. Even with a refuted clause on the row
        // it publishes nothing, because nothing landed.
        l.record_landing(&row("r1", "refused", "t1", &five, vec![refuted("ad-7p85")]))
            .map_err(|e| e.to_string())?;
        let refused_publishes_nothing = l.landed_open().map_err(|e| e.to_string())?.is_empty();

        // A real landing that refutes two of them reports both.
        l.record_landing(&row(
            "l1",
            "landed-refuted",
            "t2",
            &five,
            vec![refuted("ad-epo9"), refuted("ad-fsxg")],
        ))
        .map_err(|e| e.to_string())?;
        let mut reported: Vec<String> = l
            .landed_open()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|o| o.bead)
            .collect();
        reported.sort();
        let landed_publishes_all = reported == ["ad-epo9", "ad-fsxg"];

        // A newer refusal of the same branch (main moved under it) leaves both standing.
        l.record_landing(&row("r2", "refused", "t3", &five, vec![]))
            .map_err(|e| e.to_string())?;
        let refusal_does_not_silence = l.landed_open().map_err(|e| e.to_string())?.len() == 2;
        Ok((
            refused_publishes_nothing && refusal_does_not_silence,
            landed_publishes_all,
        ))
    })()
    .unwrap_or((false, false));
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
        "fd-1",
        vec![
            "docs/absent.md says it.".into(),
            "The owner rules on the counter-argument.".into(),
            "Docs are updated.".into(),
        ],
        &ev,
    );
    let unreadable_only = judge_clauses("fd-2", vec!["The owner rules on it.".into()], &ev);
    let as_row = |j: &crate::cmd::acceptance::Judged| OpenBead {
        bead: j.bead.clone(),
        why: j.why_open(),
        refuted: j.refuted(),
        contradicted: j.why_contradicted(),
    };

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_landing(&Landing {
            id: new_id(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: "landed-refuted".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 1,
            beads: vec!["fd-1".into(), "fd-2".into()],
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
                a.worker == "fd-1"
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
            .and_then(|r| r.open_beads.iter().find(|o| o.bead == "fd-1").cloned())
            .is_some_and(|o| {
                o.why.contains("nothing Air can look up") && o.why.contains("docs/absent.md")
            });
        let unreadable_is_silent = !landed.iter().any(|a| a.worker == "fd-2");
        Ok((
            names_the_refuted_alone,
            unreadable_is_silent && row_keeps_both,
        ))
    })()
    .unwrap_or((false, false));
    let (red_fires, green_passes) = res;

    Probe {
        name: "status: landed-not-closed names only the clauses the merge contradicts; a bead Air merely could not read makes no CONTRADICTS claim",
        red_fires,
        green_passes,
    }
}

/// air-dqa: a path Air read out of prose and got wrong must not become a confident false
/// accusation. Three firings of `landed-not-closed`, zero true: adopter's clause wrote a
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

/// air-09b: a bead is a handle on a branch only while one branch carries it. adopter,
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
    let fd1 = vec!["fd-1".to_string()];
    let fd2 = vec!["fd-2".to_string()];

    // Case 1: alpha did fd-1 and has waited longest; lane batched it and carries fd-2 too.
    let both_ready = vec![
        landing("alpha", "fd-1", 30, None),
        landing("lane", "fd-1", 5, None),
        landing("lane", "fd-2", 5, None),
    ];
    let case1 = matches!(
        resolve(&fd1, &none, &both_ready, &[], &[]),
        Err(m) if m.contains("--worker alpha") && m.contains("--worker lane")
    );
    // Case 2: alpha's branch is behind main now; lane can land.
    let blocked = vec![landing("alpha", "fd-1", 30, Some("does not contain main"))];
    let lane_ready = vec![
        landing("lane", "fd-1", 5, None),
        landing("lane", "fd-2", 5, None),
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
    let single_ready = matches!(
        resolve(&fd2, &none, &lane_ready, &blocked, &[]),
        Ok(v) if v.len() == 1 && v.first().is_some_and(|l| l.worker == "lane")
    );

    Probe {
        name: "land: a bead on two branches is refused naming each with --worker; --worker lands that branch with every bead it carries; a bead on one blocked branch is still refused with its fix",
        red_fires: case1 && case2,
        green_passes: selector && single_blocked && single_ready,
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

    let want: Vec<String> = ["fd-1", "zz-nope", "fd-2"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let issue = |id: &str| air_bd::Issue {
        id: id.to_string(),
        ..Default::default()
    };
    let red = missing_ids(&want, &[issue("fd-1"), issue("fd-2")]) == ["zz-nope"];
    let green = missing_ids(&want, &[issue("fd-1"), issue("zz-nope"), issue("fd-2")]).is_empty();
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
        let ids: Vec<String> = (1..=10).map(|i| format!("fd-{i}")).collect();
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
/// any of them. With `AIR_PROJECT=air`, a PreToolUse call for `tmux kill-session -t fd-worker1`
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
    .unwrap_or((false, false));
    Probe {
        name: "project: the session's project comes from what it is told, not from ambient AIR_PROJECT",
        red_fires: res.0,
        green_passes: res.1,
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

/// air-tdc: `air worker --task` from a socket stdin (the coordinator's Bash tool) must not
/// exec `claude --tmux` (tcgetattr fails there). Red: the socket case is routed away from
/// exec. Green: a detached tmux session is actually created (pure check only when tmux is
/// absent; the probe name says so).
fn probe_launch_no_tty() -> Probe {
    use crate::cmd::launch::{Launch, launch_mode, tmux_detached_argv};
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
    let argv = tmux_detached_argv(
        name,
        Path::new("/"),
        Some(&socket),
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
    let (red, green) = res.unwrap_or((false, false));
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
        l.record_claim("fd-1", "probe", &[], "t0")
            .map_err(|e| e.to_string())?;
        let cmd = "bd update fd-1 -s awaiting_review";
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
        })
        .map_err(|e| e.to_string())?;
        let green = handover_gate(&l, "probe", &dir, cmd, true)?;
        let green_passes = matches!(green.outcome, HookOutcome::Allow { context: None });
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red_fires, green_passes))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "gate: AIR_ENFORCE=1 denies bd update -s awaiting_review without green at HEAD (names the fix); allows with green",
        red_fires: red,
        green_passes: green,
    }
}

/// Check 4: a hand-over names a bead the worker does not hold → missing `claim`.
fn probe_gate_claim() -> Probe {
    let mut red = base_facts();
    red.bead = Some("fd-1".into());
    red.bead_claimed_by_worker = false;
    let mut green = base_facts();
    green.bead = Some("fd-1".into());
    green.bead_claimed_by_worker = true;
    Probe {
        name: "gate: claim required for the named bead",
        red_fires: handover_verdict(&red)
            .missing
            .iter()
            .any(|m| m.check == "claim"),
        green_passes: handover_verdict(&green).pass,
    }
}

/// The ledger half of `air claim`: a second worker finds the open claim; the same worker
/// re-claiming after release gets a fresh row.
fn probe_claim_cas() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("fd-1", "w1", &[], "t0")
            .map_err(|e| e.to_string())?;
        let held_by_other = l
            .open_claim("fd-1")
            .map_err(|e| e.to_string())?
            .is_some_and(|c| c.worker != "w2");
        l.release_claim("fd-1", "w1", "abandoned", "t1")
            .map_err(|e| e.to_string())?;
        let free = l.open_claim("fd-1").map_err(|e| e.to_string())?.is_none();
        Ok((held_by_other, free))
    })();
    let (red, green) = res.unwrap_or((false, false));
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
/// adopter's ad-m8v1 is the reason — their log-cap probe asserted 45 against a cap the owner
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
/// breaking it — adopter's second control made visible in the output.
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
/// adopter's coordinator spent a setup window diagnosing. Green: the close releases the row
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
        l.record_claim("fd-1", "w", &[], "2026-08-20T11:00:00Z")
            .map_err(|e| e.to_string())?;
        l.stamp_handover("fd-1", "w", "2026-08-20T11:50:00Z")
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
        let handover_keeps_it = closes_bead("bd update fd-1 -s awaiting_review").is_none();
        // The close, as the PostToolUse arm applies it.
        let bead = closes_bead("bd close fd-1 --reason done").ok_or("close not recognised")?;
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
            .any(|c| c.bead == "fd-1");
        Ok((
            before.contains(&"handover-not-green"),
            handover_keeps_it && released && !after.contains(&"handover-not-green") && !still_held,
        ))
    })()
    .unwrap_or((false, false));
    Probe {
        name: "claim: a closed bead stops alarming; awaiting_review still holds it",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-0j4: a worker's HEAD is one sha, so every claim it holds is not-green for the same
/// reason and the same fix. adopter's `air status` printed eleven `handover-not-green` lines
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
            claim("fd-1", "2026-08-20T11:00:00Z"),
            claim("fd-2", "2026-08-20T11:30:00Z"),
            claim("fd-3", "2026-08-20T11:40:00Z"),
        ]),
        now,
        Thresholds::default(),
    );
    let one = attention(
        &snap(vec![claim("fd-1", "2026-08-20T11:00:00Z")]),
        now,
        Thresholds::default(),
    );
    Probe {
        name: "attention: three stuck claims on one worker are one line, not three",
        red_fires: three.len() == 1
            && three.first().is_some_and(|a| {
                a.kind == "handover-not-green"
                    && ["fd-1", "fd-2", "fd-3"]
                        .iter()
                        .all(|b| a.detail.contains(b))
                    && a.detail.contains("3 attempts in total")
            }),
        green_passes: one.len() == 1
            && one
                .first()
                .is_some_and(|a| a.detail.starts_with("fd-1 handed over 1 time(s)")),
    }
}

/// air-p61: `air status`'s bd budget was a flat 2 s, chosen before anything measured bd. bd's
/// measured p99 here is 1644 ms — 356 ms of headroom — and adopter's MEDIAN is 1760 ms,
/// above the whole budget, so their status reconcile timed out on ordinary calls.
///
/// Red: at adopter's measured median the budget rises above it, instead of sitting under it.
/// Green: it never exceeds the cap that keeps `air status` inside the MCP tool budget
/// (air-19u), and a ledger with no measurement yet keeps the old floor.
///
/// No number here is a second copy of a rule: the two inputs are measurements from the two
/// repos' event logs, and both assertions are relations (`>`, `<=`) rather than equalities
/// against a constant, so moving the multiplier cannot silently silence this (air-jc0).
fn probe_status_bd_budget_follows_the_measurement() -> Probe {
    use crate::cmd::bd_latency::status_bd_budget;
    let ms = |d: std::time::Duration| u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
    // Measured medians: adopter 1760 ms over 260,601 calls; this repo 1430 ms over 495,892.
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
/// adopter saw six of those in a day while the simulator and API were genuinely running.
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
/// that last one is every repo running Air today, adopter included, and refusing it would
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
        kind: "stuck",
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
        last_green_sha: None,
        main_is_ancestor: true,
        main_sha: "fedcba9876543210".into(),
        main_moved: None,
        bead_claimed_by_worker: true,
        runs_at_head: (1, 0),
        digest_present: None,
        digest_dir: None,
        bead: None,
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

/// air-4up: eight refusals in one adopter round, all caused by a coordinator landing, every
/// one worded as a defect in the worker's tree. Red: with a landing on record that HEAD does
/// not contain, the refusal names it — the sha, how long ago, whose branch — and keeps the
/// phrase adopter counts refusals by. Green: the fix is unchanged, and with no landing to
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

/// air-5wq: `handover ok: w2 at 3c39883` reads as a clearance and is a snapshot. adopter
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

fn probe_handover_matcher() -> Probe {
    Probe {
        name: "hook: handover command matcher",
        red_fires: is_handover_command("bd close fd-1")
            && is_handover_command("bd update fd-1 -s awaiting_review"),
        green_passes: !is_handover_command("git commit -am wip")
            && !is_handover_command("bd update fd-1 --claim"),
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
    let (red, green) = ok.unwrap_or((false, false));
    Probe {
        name: "ledger: verify_runs round-trip",
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
    let (red, green) = res.unwrap_or((false, false));
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
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "git: is-ancestor exit codes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-er0: the `--task` text must not be in the worker process's command line. adopter's
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
        let git = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["init", "-q", "-b", "main"])
            .output()
            .map_err(|e| e.to_string())?;
        if !git.status.success() {
            return Err(String::from_utf8_lossy(&git.stderr).to_string());
        }
        // The stub records its argv in a file rather than on stdout: without a tty (this
        // probe under `air record verify`, a Bash tool) the launcher starts the stub inside a
        // detached tmux session (air-tdc), where stdout is the pane. With a tty it execs
        // the stub directly. Either way the file appears; the socket keeps tmux private.
        let stub = dir.join("claude-stub");
        let argv_file = dir.join("argv");
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\0' \"$@\" > {}.tmp && mv {}.tmp {}\n",
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
        let out = Command::new(exe)
            .current_dir(&dir)
            .env("AIR_CLAUDE_BIN", &stub)
            .env("AIR_TMUX_SOCKET", &socket)
            .env_remove("AIR_TMUX_MODE")
            .args(["worker", "w", "--task", task])
            .output()
            .map_err(|e| e.to_string())?;
        let mut raw = None;
        // Up to 10 s: a fresh executable's first exec can take seconds on macOS.
        for _ in 0..1000 {
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
        Ok(!argv.iter().any(|a| a.contains(marker))
            && argv.first().is_some_and(|a| *a == prompt)
            && task_is_prompt(&argv, &prompt)
            && on_disk == format!("{task}\n"))
    })()
    .unwrap_or(false);
    Probe {
        name: "launch: --task reaches claude as the prompt by file; the task text is not in argv",
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
    let cached: Vec<String> = ["ad-free", "ad-owner", "ad-taken"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // Red: nudging from the cache offers all three, including the two that moved.
    let red = stop_nudge("worker", false, &cached, false)
        .is_some_and(|m| m.contains("ad-owner") && m.contains("ad-taken"));

    // Live bd: the peer's claim left `bd ready` entirely; the owner label did not.
    let live = [
        issue("ad-free", "open", &[]),
        issue("ad-owner", "open", &["owner"]),
    ];
    let confirmed = claimable(&live);
    let green = confirmed == ["ad-free"]
        && stop_nudge("worker", false, &confirmed, false).is_some_and(|m| {
            m.contains("air claim ad-free") && !m.contains("ad-owner") && !m.contains("ad-taken")
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
    let ready = vec!["fd-1".to_string()];
    let red = stop_nudge("worker", false, &ready, false).is_some();
    let once =
        stop_nudge("worker", false, &ready, false).is_some_and(|r| r.contains("air claim fd-1"));
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
/// The failure: adopter's coordinator invalidated three workers' verifies in one round by
/// landing under them, with nothing to consult. A full verify is ~420 s there and the landing
/// rate is faster, so their answer was a hand protocol (worker warns, coordinator holds).
///
/// Red: with one run in flight, `air status` prints a line naming the worker and `air land`
/// warns. Green: with nothing running both are silent, and a run whose process died is not
/// running — the reader prunes it rather than leaving a row nobody can clear.
fn probe_verify_in_flight() -> Probe {
    use crate::cmd::land::in_flight_warnings;
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
    let warned = in_flight_warnings(&[flight("alpha", Some(1))], at);
    let red = shown.contains("verify in flight: alpha")
        // Elapsed in seconds: rounding a just-started run to "0 min" is what makes it look
        // ignorable, and 420 s is the number that decided this bead.
        && shown.contains("420s")
        && warned.len() == 1
        && warned
            .first()
            .is_some_and(|w| w.contains("alpha") && w.contains("invalidates it"));

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
        && in_flight_warnings(&[], at).is_empty()
        && pruned;
    Probe {
        name: "verify: a run in flight is named by status and warned about by land; nothing running is silent and a dead pid clears",
        red_fires: red,
        green_passes: green,
    }
}

/// air-bxe: a landing has a state, and `air status` holds it.
///
/// The failure: adopter's coordinator reported a land done three times before the process
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
