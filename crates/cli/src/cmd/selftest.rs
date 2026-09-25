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
/// A `Write` a probe can read back (air-e21v). `run_tee` pumps a child's streams to two
/// sinks; a probe must never pass this process's stdout, because `air selftest --json` writes
/// its array there and anything else on that stream stops it parsing.
#[derive(Debug, Clone, Default)]
struct Shared(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Shared {
    fn new() -> Self {
        Self::default()
    }
    fn text(&self) -> String {
        self.0
            .lock()
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_default()
    }
}

impl std::io::Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut b) = self.0.lock() {
            b.extend_from_slice(buf);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

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
/// 1. **A mutant that does not build.** The adopter's first run scored 15 of 15 red; two were a
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
    // The lease-needed gate. The anchor makes ANY holder count as this session's, which is
    // the one comparison the gate exists to make: the second store the adopter's guard read
    // answered exactly this question wrongly. Red falls (w2's lease lets w1 run `make api`);
    // green stands, because every green case is either unmatched, the owner, or really held.
    (
        "lease: a declared command from a session without the lease is refused (advised unenforced)",
        Mutation {
            file: "crates/cli/src/cmd/lease.rs",
            from: "Some(l) if l.worker == me => match defect(&l) {",
            to: "Some(l) if l.worker == me || !me.is_empty() => match defect(&l) {",
            also_red: &[],
        },
    ),
    // Precheck (2026-09-25): the one arm that reads the precheck, inverted rather than deleted, so the
    // undeclared path (the green half) is untouched and only the declared refusal falls.
    (
        "status: where the repo declares a precheck, batch-ready wants a green one at the head (`no-precheck` names the command); undeclared, the rule is unchanged",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: "if f.precheck_required && !f.precheck_green_at_head {",
            to: "if f.precheck_required && f.precheck_green_at_head {",
            also_red: &[],
        },
    ),
    // The collapse the kind exists to prevent: a precheck row written under the verify kind.
    // The idle probe falls with it, seen: its in-flight line then reads "verify in flight".
    (
        "ledger: a green precheck is found as a precheck and never as a verify green, by commit, by tree, or as a batch candidate",
        Mutation {
            file: "crates/ledger/src/verify.rs",
            from: "Kind::Precheck => \"precheck\",",
            to: "Kind::Precheck => \"verify\",",
            also_red: &[
                "attention: idle-without-claim is silent for a worker whose own verify is in flight",
            ],
        },
    ),
    // The cut as a program (2026-09-25): without the order rule, whichever member was typed first survives a
    // pairwise conflict. Anchored on the one call, not the whole drop rule.
    (
        "batch cut: a pairwise conflict drops the later-ready member, naming the other side and the paths, whatever order the members arrive in",
        Mutation {
            file: "crates/cli/src/cmd/batch_cut.rs",
            from: "    order(&mut cands);\n",
            to: "    let _ = &mut cands;\n",
            also_red: &[],
        },
    ),
    // air-88av. The anchor widens the lookup from "unmerged" to "everything but a deletion" —
    // lowercase in `--diff-filter` EXCLUDES, so `d` matches every modified path. Verified in a
    // fixture rather than reasoned about, after my first comment here claimed the opposite.
    //
    // So it neutralises the DISTINCTION rather than the refusal, and the half that falls is the
    // GREEN one: a merely dirty tree starts being refused. That is deliberate and it is the
    // better target. "Do not turn this into a dirty-tree refusal" is the clause most at risk —
    // `air record` is meant to work on a dirty tree, the `dirty` column exists for it, and a
    // collapse there breaks the normal case in a way a green suite would call correct. The red
    // half survives, because a conflicted tree is still refused; what is lost is that a clean-
    // but-dirty one is not.
    //
    // Anchored on the LOOKUP rather than on the refusal branch: mutating the refusal would
    // leave a probe checking whether Air says something, and this bead is about whether Air
    // asks the right question.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "record: an unresolved merge is refused with its paths and no row written, while a dirty tree and a resolved-but-uncommitted merge both still record",
        Mutation {
            file: "crates/cli/src/git.rs",
            from: "run(cwd, &[\"diff\", \"--name-only\", \"--diff-filter=U\"])",
            to: "run(cwd, &[\"diff\", \"--name-only\", \"--diff-filter=d\"])",
            also_red: &[],
        },
    ),
    // air-6dj4. The anchor records an EMPTY SHA where there is no head, which is the exact
    // shape the bead named as the branch this could get wrong quietly. It compiles, the row is
    // written, `head_sha` is set, and the inbox prints " at " followed by nothing — a commit
    // nobody can look up, with every other check passing. Nothing downstream distinguishes it
    // from a real head, which is why the probe asserts the absence carries a REASON rather than
    // asserting that something was recorded.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "capture: the row records the worktree's real head, and a capture written where there is no head says so with a reason rather than an empty sha",
        Mutation {
            file: "crates/cli/src/cmd/capture.rs",
            from: "Err(e) => air_ledger::captures::Head::Absent(format!(\"git rev-parse HEAD: {e}\")),",
            to: "Err(_) => air_ledger::captures::Head::At(String::new()),",
            also_red: &[],
        },
    ),
    // air-lyjr. The anchor truncates the close reason at 500 chars — a generous command line,
    // and the same neutralisation air-45pw declared for capture, but anchored in close.rs so it
    // reaches THIS probe rather than both. Under it the reason still records, still reads long,
    // still closes both beads, and passes any check that asks whether a reason is there. Only
    // the byte-for-byte and length assertions catch it, which is the clause this bead was
    // written around. The refusals and the inline route survive, so the green half holds.
    // `get(..500)` rather than a slice: a mutation that panics is a crash, not a red.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "close: --reason-file records the file whole for every bead named, and an unreadable path is an error rather than an empty reason",
        Mutation {
            file: "crates/cli/src/cmd/close.rs",
            from: "    let reason = reason.as_str();",
            to: "    let reason = reason.get(..500).unwrap_or(reason.as_str());",
            also_red: &[],
        },
    ),
    // air-gazh. The anchor widens the ledger half from "bd said this bead is CLOSED" to "the
    // claim ended somehow", which is the failure that would make this condition worthless
    // rather than absent: every landed bead has a released claim, so the line would fire on
    // ordinary completed work and a reader would learn to skip it. It compiles, the condition
    // still renders, and the quiet case is still quiet — only the discrimination goes, which
    // is the rule the probe names.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "status: a closed bead whose commits reached no tree but its author's worktree is named, and a claim that merely ended is not",
        Mutation {
            file: "crates/ledger/src/claims.rs",
            from: "WHERE release_reason = 'closed' ORDER BY released_at DESC",
            to: "WHERE released_at IS NOT NULL ORDER BY released_at DESC",
            also_red: &[],
        },
    ),
    // air-jy99. The anchor RESTORES the prescription, in `land.rs` rather than `status.rs`,
    // deliberately: the probe checks both renderers and a mutation in either must take it red.
    // Restoring it in the renderer that is NOT the bead's headline surface is the stronger
    // evidence, because that is the shape air-155w's own defect came back through — one
    // renderer corrected, the other left teaching the forbidden thing. The rest of the
    // sentence survives, so the mutation reaches the prescription and not the whole line.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "status/land: an undischarged clause names the condition and no flow-dependent action, in both renderers",
        Mutation {
            file: "crates/cli/src/cmd/land.rs",
            from: "read the bead, then see that \\\n     nothing left over is untracked.",
            to: "read the bead, then either reopen it or file what is left.",
            also_red: &[],
        },
    ),
    // air-45pw. The anchor truncates the file route at 500 chars — a generous command line, and
    // exactly the shape of the bug: the capture is still filed, still long, still reads as a
    // capture. A probe asserting "non-empty" or "long enough" stays green here, which is why the
    // green half asserts byte-for-byte equality with the file. The refusals, the positional
    // route and the ledger write all survive it, so it reaches one branch rather than the guard.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "capture: --file files the finding whole, byte for byte, and passing both routes or neither is refused naming both",
        Mutation {
            file: "crates/cli/src/cmd/capture.rs",
            from: "Ok(s) => Ok(s.trim().to_string()),",
            to: "Ok(s) => Ok(s.trim().chars().take(500).collect()),",
            also_red: &[],
        },
    ),
    // air-6wv2. The anchor removes the DISTINCTION and nothing else, per the bead: the refusal
    // still names the assignee, the bd version rule and the fixing command, the claims lookup
    // still runs, and the sentence is still printed — it just always reads as live work. What
    // goes is the only thing this bead added.
    //
    // The sentence is an inline literal precisely so it can be deleted without leaving a
    // binding unused: a mutation that does not build is reported BROKEN rather than red, which
    // is evidence of nothing. Nothing is anchored on the ledger check that makes the sentence
    // true — that is the green half's subject and must survive.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "claim: the assignee refusal says Air has no claim behind that assignee and offers a reopen as the cause, and live work is refused earlier by name",
        Mutation {
            file: "crates/cli/src/cmd/claim.rs",
            from: "Air has no open claim behind that assignee, so it may be left over rather than live work: bd keeps an assignee through a close, and a reopened bead can come back pencilled in with nobody having assigned it. ",
            to: "",
            also_red: &[],
        },
    ),
    // air-vsvt (reopened). The anchor is the line that makes the parent list mean "what the
    // batch merged": the first parent of each merge is the lane's own line, and every other is
    // a branch it took. Under `skip(1)` the lane's own history joins the member set, so the
    // recorded shas stop being the ones the batch merged, which is the rule this probe names.
    // The name resolution, the unattributed fallback and the lane exclusion all survive it, so
    // the green half holds and the mutation reaches one branch rather than removing the guard.
    // It does NOT restore the pre-fix defect and is not claimed to; it neutralises the rule.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "batch: a run records the shas its batch merged, keeping one whose branch has moved off it, unattributed rather than dropped",
        Mutation {
            file: "crates/cli/src/cmd/batch.rs",
            from: "for p in line.split_whitespace().skip(2) {",
            to: "for p in line.split_whitespace().skip(1) {",
            // Declared after RUNNING the mutation and seeing this one fall too, not predicted.
            // It genuinely shares the rule: both probes assert the recorded member list, and
            // "which parents are members" is the single fact underneath both. Declaring a
            // shared red is a claim about the rule and deserves the same scrutiny as the probe
            // — it is also how a flake gets laundered into a permanent shared-rule assertion —
            // so the test applied here was whether the OTHER probe would still be checking
            // something true if this rule were removed. It would not.
            also_red: &[
                "batch: a landing records exactly the branches its batch merged, once each, and never one already in main",
            ],
        },
    ),
    // air-htmn. The anchor restores the original bug exactly: a completed fast-forward
    // reported as a refusal. ONE half, per alerts' rule — the genuinely-not-moved case and
    // the cannot-tell case both still answer correctly under it, so what it isolates is
    // precisely whether Air reports a landing that happened as one that did not.
    (
        "land: a fast-forward that completed is not reported as untouched, and a look that failed is not reported as a refusal",
        Mutation {
            file: "crates/cli/src/cmd/land.rs",
            from: "        Some(true) => FfVerdict::Landed,",
            to: "        Some(true) => FfVerdict::Refused(err.to_string()),",
            also_red: &[],
        },
    ),
    // air-kexg. The anchor is the permitting half alone: under it no range is ever
    // journal-only, so a branch of journal entries is refused again and the defect returns.
    // The CONSTRAINT survives it untouched - a mixed range still needs a bead either way -
    // which is deliberate, because the constraint is the half that would still look right if
    // it rotted, and a mutation taking out both would not tell the two apart. The refusal
    // wording, the Option on Landing and every other branch's behaviour also survive.
    // Anchor as rustfmt leaves it, per air-gei.
    (
        "land: a branch whose only commits are session-journal entries lands with no bead, and a range mixing them with anything else still needs one",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: ".all(|p| p.strip_prefix(dir).is_some_and(|r| r.starts_with('/')))",
            to: ".all(|_| false)",
            also_red: &[],
        },
    ),
    // air-33rn. ONE half, which is alerts' caution from air-kexg: a mutation taking out both
    // goes red for the right reason by accident and cannot distinguish coverage of the
    // constraint from coverage of the permission.
    //
    // My first anchor was `landings.first()`, making every worker read as landable — and it
    // took out both halves, because a worker selection never considered then reads as landable
    // too, which is the green half. Caught by running it, not by reading it. This one silences
    // only the refusal lookup: the journal-only and no-green cases stop being reported, while
    // landable, never-considered and cannot-tell all still answer correctly. What it isolates
    // is exactly whether a worker is told its branch will NOT land, the bead's subject.
    (
        "handover: a worker is told what the landing gate would say about its own branch, read from select rather than recomputed",
        Mutation {
            file: "crates/cli/src/cmd/handover.rs",
            from: "    if let Some(s) = sel.skipped.iter().find(|s| s.worker == worker) {",
            to: "    if let Some(s) = sel.skipped.iter().find(|_| false) {",
            also_red: &[],
        },
    ),
    // air-i6fd. The anchor restores the defect exactly: main's tip read from the running cwd's
    // HEAD instead of from the ref. Everything else survives — the green check, the skipped
    // entries, the bead attribution, the error paths — so what it isolates is whether the
    // answer depends on WHERE the command ran. Under it the running worker's branch compares
    // against itself and leaves through the one silent exit, which is invisible to any check
    // that asks whether `landable` has entries rather than whether it has the RIGHT ones.
    // Anchored on `select`'s call rather than on `git::main_tip` itself: mutating the helper
    // would also take out the batch-ready path and the rewound check, which is a mutation that
    // removes the guard rather than one that reaches a branch (air-682).
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "status: landable is the same from a worktree as from the main checkout, and the running worker's own branch is never silently dropped",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: "    let main_tip = match git::main_tip(repo) {\n        Ok(t) => t,\n        Err(e) => {\n            out.errors.push(format!(\"git rev-parse main: {e}\"));\n            return out;\n        }\n    };",
            to: "    let main_tip = git::head(repo).unwrap_or_default();",
            also_red: &[],
        },
    ),
    // air-72t7. The anchor restores the boundary that dropped two of select's three fields.
    // `landable` still fills, every skip still carries what it compared, and the human
    // rendering is untouched — so what it isolates is exactly whether a reader of `--json`
    // can tell 'nothing to land' from 'I could not tell'.
    (
        "status: --json says why each branch cannot land and distinguishes an error from an empty queue",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: "        land_skipped: selection.skipped,",
            to: "        land_skipped: Vec::new(),",
            also_red: &[],
        },
    ),
    // air-3xww. The anchor hard-codes the journal's path, which is the one way this can go
    // wrong quietly: the directory stays scaffolded and a repo that configured another gets
    // a README in a place it does not use. The created-only-when-absent rule and the
    // no-gate assertion both survive it, so what it isolates is whether the location is
    // really the repo's.
    (
        "journal: air init scaffolds the session journal where the other scaffolded items go, at the configured path, and no gate reads it",
        Mutation {
            file: "crates/cli/src/cmd/init.rs",
            from: "                path: format!(\"{}/README.md\", journal.0),\n                create: true,",
            to: "                path: format!(\"{DEFAULT_JOURNAL_DIR}/README.md\"),\n                create: true,",
            also_red: &[],
        },
    ),
    // air-rud0. The anchor is the discharged line's format string and nothing else: the
    // judgement, the two honest branches and `--json`'s `how` all survive it, because none of
    // them moved. Under it the tick is bare again and a reader scanning a nine-bead landing's
    // verdict column sees `ok` with nothing beside it — which is the defect exactly, and one
    // that is RIGHT in most cases, so a probe that stayed green under this was checking that
    // discharged clauses appear rather than that they say what discharged them.
    // Anchor as rustfmt leaves it, per air-gei.
    (
        "land: a discharged acceptance clause names the lookup that discharged it on the verdict line, and the judgement is still a lookup rather than a reading",
        Mutation {
            file: "crates/cli/src/cmd/acceptance.rs",
            from: "s.push_str(&format!(\"    ok ({how}) — {text}\\n\"));",
            to: "s.push_str(&format!(\"    ok   {text}\\n         {how}\\n\"));",
            also_red: &[],
        },
    ),
    // air-hgi9. The anchor is the arm that renders the reason, and nothing else: the counts in
    // `cover`, both pure renderers, the batch-predates arm and the flaky arm all survive it.
    // Under it the four not-green states collapse back into the one sentence they shared, with
    // the generic fix — which is the defect, and it is invisible in any check that asks whether
    // the gate says SOMETHING. The green half is expected to survive: a covering green still
    // passes and a partial one still names the commit it lacks.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "gate: each not-green state names the fact that distinguishes it, and the two with opposite responses carry different fixes",
        Mutation {
            file: "crates/hooks/src/gate.rs",
            from: "        } else if let Some(why) = f.batch_absent.as_deref() {",
            to: "        } else if let Some(why) = None::<&str> {",
            also_red: &[],
        },
    ),
    // air-vsvt. The anchor is the git call that PRODUCES the recorded sha, because that is
    // what the probe exercises: it asserts over `git::merge_base` on a real history, not over
    // `members_of`, whose worktree enumeration a probe cannot cheaply stand up. Under this the
    // answer is the branch's head again — where it is now, rather than what the batch took —
    // which is exactly the adopter's symptom: a branch that commits after the lane merges it
    // drops out of its own batch's record, for good.
    //
    // Anchored here rather than on `members_of`'s own line on purpose: a mutation must reach
    // what the probe asserts (air-682), and mutating `members_of` would have left this probe
    // green while looking like it covered it. `members_of` end to end is pinned by
    // `a_batchs_recorded_members_are_the_shas_it_took_not_where_the_branches_moved_to`.
    // Anchor as rustfmt leaves it, per air-gei.
    (
        "batch: a run records the sha the batch TOOK from each branch, which does not move when that branch does, and the red line reports exactly what was recorded",
        Mutation {
            file: "crates/cli/src/git.rs",
            from: "run(cwd, &[\"merge-base\", a, b])",
            to: "run(cwd, &[\"rev-parse\", a])",
            also_red: &[],
        },
    ),
    // air-zqmi. The anchor restores the unconditional stamp, which is the defect exactly:
    // every hand-over command the gate saw counted, passes included. The refusal still
    // counts and the gate still decides, so what it isolates is whether a SUCCESS is
    // recorded as a failure — the thing the channel then reported to the whole fleet.
    (
        "handover: only a hand-over the gate refused counts as an attempt, and one that passes clears the count",
        Mutation {
            file: "crates/cli/src/cmd/hook.rs",
            from: "    let stamped = match (&bead, v.pass) {",
            to: "    let stamped = match (&bead, false) {",
            also_red: &[],
        },
    ),
    // air-et0o. The anchor is the render alone. The query, the change-only fingerprint, the
    // wording of the warning and the once-per-session suppression all survive it, so what it
    // isolates is exactly whether the sentence dates the entry — which is the whole bead: a
    // fortnight-old journal row and a live concurrent edit were spelled identically. The green
    // half is expected to SURVIVE this, because a peer is still warned about either way.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "hook: the peer warning dates each holder's journal entry, so a fortnight-old one does not read like a live edit",
        Mutation {
            file: "crates/cli/src/cmd/hook.rs",
            from: "peer_ages(&peers, &now())",
            to: "peers.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(\", \")",
            also_red: &[],
        },
    ),
    // air-155w. The anchor restores the exact string the adopter was given. The diagnosis,
    // its dating and every other check survive it, so what it isolates is precisely whether
    // a refusal hands a lane worker the clause its flow forbids.
    (
        "gate: no flow-dependent fix tells a worker to record a verify, and the main-moved diagnosis is unchanged",
        Mutation {
            file: "crates/hooks/src/gate.rs",
            from: "fix: \"git merge main, then a green at the new head (your own, or your lane's)\"",
            to: "fix: \"git merge main && air record verify -- make verify\"",
            // Declared from RUNNING the mutation, not from reading: both of these assert the
            // same rule from a different surface, so they fall with it legitimately. Left
            // undeclared they would have made this VACUOUS, which is the trap the revived
            // `--prove` caught three times an hour before this bead (air-e21v).
            also_red: &[
                "gate: a refusal after a landing names the landing that moved main, when and from whom, and its fix asserts no repair a verify lane forbids",
                "hook: the Stop advisory never tells a worker to merge main or record a verify, and names `air handover` instead; a flow-free fix is still printed in full",
            ],
        },
    ),
    // air-dwq5. The anchor is the format string of the version LINE alone: the JSON, the four
    // surfaces and the surface version all survive it. Under it the line prints the crate
    // version and nothing else, which is what shipped for a round — and it still looks exactly
    // like a version, which is why nobody noticed that two binaries with different behaviour
    // were reporting the same string. Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "version: air says which binary it is — the build reaches --version, --version --json is JSON, and doctor and status carry the same object",
        Mutation {
            file: "crates/cli/src/cmd/install.rs",
            from: "        \"air {} (built from {}, surface {})\",\n        env!(\"CARGO_PKG_VERSION\"),\n        BUILD,\n        SURFACE_VERSION",
            to: "        \"air {}\",\n        env!(\"CARGO_PKG_VERSION\")",
            also_red: &[],
        },
    ),
    // air-x1ha. The anchor is the arm for "bd never had this id", and nothing else: recording
    // the resolved id at claim time, releasing a bead bd knows and no longer holds, and the
    // reported line all survive it. Under it that arm releases the row again, which is the
    // defect — a claim under an id bd cannot resolve is dropped while the work continues.
    // Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "claim: a prefix claim is recorded under the id bd resolved and survives the reconcile; a bead bd no longer holds is still released",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: "Some(None) => unresolved.push(format!(\"{} ({})\", c.bead, c.worker)),",
            to: "Some(None) => {\n                        let _ = ledger.release_claim(&c.bead, &c.worker, \"reconciled\", &at);\n                        continue;\n                    }",
            also_red: &[],
        },
    ),
    // air-e21v. The anchor puts a probe's output back on this process's stdout, which is
    // exactly what broke `--prove`: the tail still carries both streams, every other probe
    // still passes, and only the machine-readable output stops parsing. A probe that stayed
    // green under it would be reading the array out of the middle of the stream rather than
    // from its first byte, which is not what a parser does.
    (
        "record: a red run's output is kept, bounded by its tail and by a count of logs, and a green run's is not",
        Mutation {
            file: "crates/cli/src/cmd/selftest.rs",
            // NOT `std::io::stdout(), std::io::stderr()`, which is the original bug: it
            // pollutes the child's `--json`, so `build_and_run` cannot parse it and the row
            // reads BROKEN rather than PROVEN. **A mutation that breaks the channel `--prove`
            // reads cannot be proven by `--prove`** — measured, 2026-09-06, and the reason the
            // anchor is this one instead. Both streams into ONE sink breaks the same rule (each
            // stream reaches its own sink and not the other's) and leaves the reader intact.
            from: "        out_sink.clone(),\n        err_sink.clone(),",
            to: "        out_sink.clone(),\n        out_sink.clone(),",
            also_red: &[],
        },
    ),
    // air-avj. The anchor is the branch that decides what a Stop advisory prints, and nothing
    // else: the facts, the pointer, the flow-free fix and `air handover`'s own message all
    // survive it. Under it every repair is printed again, including the two a verify lane
    // exists to stop a worker doing — which is the defect exactly, so a probe that stayed
    // green under this was checking that the hook says SOMETHING rather than that it stopped
    // saying the wrong thing. Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "hook: the Stop advisory never tells a worker to merge main or record a verify, and names `air handover` instead; a flow-free fix is still printed in full",
        Mutation {
            file: "crates/hooks/src/gate.rs",
            // Re-anchored (air-155w): the refusal renderer gained its own
            // `if m.flow_dependent {` when flow-dependent fixes became conditions, so the
            // one-line anchor matched twice and read as ambiguous. This one names the arm that
            // is unique to `stop_message` — the check-and-detail with no fix, which is exactly
            // what the Stop advisory drops the repair for.
            from: "                format!(\"{}: {}\", m.check, m.detail)",
            to: "                format!(\"{}: {} — run `{}`\", m.check, m.detail, m.fix)",
            also_red: &[],
        },
    ),
    // air-3jv5. The anchor is the join condition alone. The status line, the seeding and
    // the leave all survive it, so what it isolates is exactly whether a row no session is
    // behind gets announced to the fleet as a worker arriving — which is what happened.
    (
        "sessions: a row no session is behind is named in status and never announced as a worker joining, and a real one still is",
        Mutation {
            file: "crates/cli/src/cmd/mcp.rs",
            from: "if !prev.contains(&s.session_id) && s.has_transcript {",
            to: "if !prev.contains(&s.session_id) {",
            also_red: &[],
        },
    ),
    // air-5ik. The anchor is the keep-or-not test alone: the tail, the prune, the ceiling and
    // the end-to-end write all survive it, so what changes is only WHICH runs write a log.
    // Under it a green writes one too, and since the store is bounded by COUNT rather than by
    // age, a fleet's greens evict the reds the store exists for. Anchor taken from the file
    // AFTER rustfmt, per air-gei.
    (
        "record: a red run's output is kept, bounded by its tail and by a count of logs, and a green run's is not",
        Mutation {
            file: "crates/cli/src/cmd/runlog.rs",
            from: "pub fn keeps_output(exit_code: i32) -> bool {\n    exit_code != 0\n}",
            to: "pub fn keeps_output(exit_code: i32) -> bool {\n    exit_code == exit_code\n}",
            also_red: &[],
        },
    ),
    // air-cyf. The anchor puts the window back, at the value it had. Everything else
    // survives: the batch is still found, supersession still decides, a plain red and a
    // killed run are still not batches. What changes is only whether a busy day can age a
    // standing red out of view, which is the failure the bead records and the one a probe
    // reading fewer than 21 later runs would not have seen.
    (
        "batch: a standing red batch is reported until a green carries every member, and is never aged out by later runs",
        Mutation {
            file: "crates/cli/src/cmd/batch.rs",
            from: "let run = ledger.latest_red_batch(Kind::Verify).ok()??;",
            to: "let run = ledger.latest_runs(Kind::Verify, 20).ok()?.into_iter().find(|r| r.verdict() == air_ledger::verify::Verdict::Red && !r.members.is_empty())?;",
            also_red: &[],
        },
    ),
    // air-btz. The anchor is the walk's bound, so the `blocks` filter, the rendering and the
    // two bd calls all survive it: what changes is only how far up the check looks. Under it
    // the parent case is still found — and bd already refuses that one on every route, so a
    // probe that stayed green under this was testing a shape that cannot occur. The whole
    // reachable subject is an ancestor two or more levels up, which is exactly what bd's own
    // dotted-id prefix test misses. Anchor taken from the file AFTER rustfmt, per air-gei.
    (
        "status: a bead blocked by its own ancestor is named with the edge and the fix; the hierarchy edge and a sibling are not, and bd is asked once per call",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            from: "for depth in 1..=parents.len()",
            to: "for depth in 1..=1",
            also_red: &[],
        },
    ),
    // air-jsz. The anchor is the one arm that turned a silent pass into a refusal. Under
    // it a repo that declares an adopter and has no names goes back to skipping, which is
    // the exact state a whole round ran in; the leak refusal and the contributor's skip
    // both survive, so a probe that stays green under it was checking that the check runs
    // rather than that it can no longer be handed nothing.
    (
        ADOPTER_CHECK_PROBE,
        Mutation {
            file: "crates/cli/src/cmd/privacy.rs",
            from: "        (true, true) => Verdict::RefuseDeclaredButNoNames,",
            to: "        (true, true) => Verdict::SkipUndeclared,",
            also_red: &[],
        },
    ),
    // air-1n3. The anchor is the one branch that separates a stop from every other
    // notification. Under it a permission prompt marks the session STOPPED, which is the
    // failure that matters: `air status` would report a session as down while it sits
    // waiting for an answer, and a coordinator acting on that wakes a session that never
    // stopped. Installing the two events, the phrasing and the declared-field read all
    // survive it.
    (
        "limit: a stopped session is recorded from the harness's own notification, and status says whether the harness is bringing it back",
        Mutation {
            file: "crates/cli/src/cmd/hook.rs",
            from: "kind == \"stop_failure\" || kind.starts_with(\"quota_auto_resume\")",
            to: "kind == \"stop_failure\" || !kind.is_empty()",
            also_red: &[],
        },
    ),
    // air-84u. The anchor is the quantifier and nothing else: the rendering, the count and the
    // ready-set filter all survive it, so a probe that stays green under `any` was checking
    // that the line exists rather than that it names only an epic with nothing open under it.
    // That is the one direction this line must not fail in — naming an epic somebody is
    // working on costs the line its credibility, and nothing refuses on it to make up for that.
    (
        "status: a ready epic with no open child is named with its closed count; one with work under it, in any status but closed, is not",
        Mutation {
            file: "crates/cli/src/cmd/status.rs",
            // The anchor is what rustfmt LEFT, not what was typed: written as one expression
            // it was reflowed onto three lines and the anchor matched nothing, which
            // `air selftest --prove` calls BROKEN and `make verify` does not check at all.
            from: "all(|c| c.status == \"closed\")",
            to: "any(|c| c.status == \"closed\")",
            also_red: &[],
        },
    ),
    // air-5nh. The anchor is the owner-gated test alone, so every count, the sort, the
    // 300 s bucket and the printed threshold survive it: what changes is only WHICH
    // population the rate reads. Under it an ordinary fast release counts as a bead the
    // worker could not start, and the number that refused two mechanisms inflates.
    (
        "audit: re-claim churn is counted from claims alone, and the rate reads the owner-gated-inside-a-minute population with its 10% threshold beside it",
        Mutation {
            file: "crates/cli/src/cmd/audit.rs",
            from: "r.contains(\"owner-gated\") || r.contains(\"owner gated\")",
            to: "r.contains(\"owner-gated\") || !r.is_empty()",
            also_red: &[],
        },
    ),
    // air-ej4. The anchor is the `exit 1` alone: the echo, the target and the scaffold's
    // created-only-when-absent rule all survive it, so a probe that stays green under this
    // was testing that a Makefile exists rather than that its verify refuses.
    (
        "init: the verify target `air init` scaffolds FAILS until it is edited, so a fresh repo cannot record a green for an empty check",
        Mutation {
            file: "crates/cli/src/cmd/init.rs",
            from: "then delete this line'; exit 1",
            to: "then delete this line'; exit 0",
            also_red: &[],
        },
    ),
    // The ledger lane's probes, 2026-08-29. Each anchor was run by hand when the probe was
    // written, and each names ONE branch: the change-only gate, the enumeration, the
    // referenced-day protection, the join's file-and-order keys, the freshness window, the
    // bookkeeping overlap, the push deny.
    (
        "attention: idle-without-claim is silent for a worker whose own verify is in flight",
        Mutation {
            // Restores the predicate exactly as it stood when it offered an adopter's lane 58
            // beads 945 s into a batch verify: every other term kept, only the one that reads
            // the in-flight field removed.
            file: "crates/cli/src/cmd/status.rs",
            from: "                            && !verify_running(s, &w.worker)\n",
            to: "",
            also_red: &[],
        },
    ),
    (
        "attention: idle-without-claim is silent for a worker whose tree has a process that is not its session, and the land warning names it",
        Mutation {
            // Restores the predicate as it stood on 2026-09-07 at an adopter: only runs Air
            // recorded count as progress, so an unrecorded precheck reads as doing nothing.
            file: "crates/cli/src/cmd/status.rs",
            from: "                            && !tree_busy(s, &w.worker)\n",
            to: "",
            also_red: &[],
        },
    ),
    (
        "handover: a member of a standing red batch is told so, and a non-member is never told it was not",
        Mutation {
            // Drops the membership test, so every worker gets the batch line whether or not
            // it was in it — including when the members list is empty, which is the exact
            // wrong answer this exists to avoid.
            file: "crates/cli/src/cmd/handover.rs",
            from: "    if !b.members.iter().any(|m| m.worker == worker) {\n        return None;\n    }\n",
            to: "",
            also_red: &[],
        },
    ),
    (
        "status: the epic count carries no instruction, and the line that knows which epics are undecomposed carries it",
        Mutation {
            // Puts the instruction back on the count of every ready epic, which is the
            // sentence the adopter audited six epics against.
            file: "crates/cli/src/cmd/status.rs",
            from: "{epics} epic(s), not claimable",
            to: "{epics} epic(s) to decompose, not claimable",
            // air-f10's probe pins the whole rendered line, wording included, so it is
            // legitimately red under this and is named rather than left to look like spread.
            also_red: &[
                "status: the ready line names epics apart from claimable work; a set of only epics and owner beads is zero claimable, and the split is exactly bd's set",
            ],
        },
    ),
    (
        "doctor: the running binary is named against the checkout it runs in, and only where that comparison means something",
        Mutation {
            // Drops the guard that decides whether this is Air's checkout, so the line fires
            // in any repo with a workspace version — which is every adopting repo, where the
            // comparison is meaningless.
            file: "crates/cli/src/cmd/doctor.rs",
            from: "    if !cli.lines().any(|l| l.trim() == r#\"name = \"air\"\"#) {\n        return None;\n    }\n",
            to: "",
            also_red: &[],
        },
    ),
    (
        "status: the overlap line names only holders that can collide, so six clean ones produce no line",
        Mutation {
            // Back to "a row exists", which is the predicate that named ten workers on a file
            // with one live editor.
            file: "crates/cli/src/cmd/status.rs",
            from: "        .filter(|h| h.uncommitted || h.committed)\n",
            to: "",
            also_red: &[],
        },
    ),
    (
        "gate: a refusal names the command it refused, and the Stop message, which refused nothing, does not",
        Mutation {
            // Back to naming the gate whatever it matched, which is the sentence that told a
            // worker its successful hand-over had failed.
            file: "crates/hooks/src/gate.rs",
            from: "        let subject = f.refused_command.as_deref().unwrap_or(\"handover\");",
            to: "        let subject = \"handover\";",
            also_red: &[],
        },
    ),
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
        "bd: every multi-id call's budget grows with the id count (acceptance read and air close), and the refusal names the count, the budget and AIR_BD_TIMEOUT_MS",
        Mutation {
            // Drop the per-id term from the ONE shared function: the budget is the base again,
            // whatever the count, which is the flat budget that refused the adopter's batch.
            // The probe's scaled read and close then time out exactly as its flat ones do.
            file: "crates/bd/src/lib.rs",
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
            to: "        HookEvent::PermissionRequest => {\n            let prev = set_session(ledger, input, worker, role, \"stuck\", None)?;\n            Dispatched::new(HookOutcome::Allow { context: None }, \"stuck\", transition(&prev, \"stuck\"))\n        }\n        _ => Dispatched::new(\n            HookOutcome::Allow { context: None },\n            \"ignored\",\n            \"no handler\",\n        ),",
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
                // air-e21v, declared from a `--prove` run rather than by reading: the
                // Stop advisory (air-avj) renders this same refusal, so it falls with the
                // rule legitimately. Undeclared it made this mutation VACUOUS.
                "hook: the Stop advisory never tells a worker to merge main or record a verify, and names `air handover` instead; a flow-free fix is still printed in full",
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
                "gate: a refusal after a landing names the landing that moved main, when and from whom, and its fix asserts no repair a verify lane forbids",
                // Both assert on the refusal this rule produces (air-75u, air-5wq; declared
                // by air-8d7).
                "hook: a session is who its launcher says, not where its shell sits; a refusal names whose tree it is about",
                "handover: the ok line names the main it checked against, and a refusal after main moves names the new one",
                // air-e21v, declared from a `--prove` run rather than by reading: the Stop
                // advisory (air-avj) renders the same refusal, so it goes red with this rule
                // legitimately. Undeclared it made this mutation VACUOUS.
                "hook: the Stop advisory never tells a worker to merge main or record a verify, and names `air handover` instead; a flow-free fix is still printed in full",
            ],
        },
    ),
    (
        "gate: a refusal after a landing names the landing that moved main, when and from whom, and its fix asserts no repair a verify lane forbids",
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
        "land and close: the role is the launcher's AIR_ROLE, so no directory and no --repo makes a worker the coordinator",
        Mutation {
            // Let every role land. The worker half of the probe falls; the coordinator and
            // owner halves stay green, which shows the anchor reaches the role gate alone.
            file: "crates/cli/src/cmd/land.rs",
            from: "    if role != \"worker\" {\n        return Ok(());\n    }\n    Err(\n        \"refused: `air land`",
            to: "    if role != \"nobody\" {\n        return Ok(());\n    }\n    Err(\n        \"refused: `air land`",
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
        "status: landed-not-closed names only the clauses whose file the merge did not change, claims no contradiction, and stays silent about a bead Air merely could not read",
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
        "hook: a worker's edit outside its worktree is denied naming the path; inside is allowed and the coordinator is never fenced, wherever it runs",
        Mutation {
            // Fence the coordinator instead of the worker: one comparison, it compiles, and
            // the fence, the path arithmetic and the message are all untouched. The worker
            // stops being fenced (the probe's RED half falls) and the coordinator still is
            // not, because its checkout IS the root it would be measured against — so the
            // GREEN half survives, which is what shows the anchor reaches the role gate alone
            // rather than taking out the check.
            file: "crates/cli/src/cmd/hook.rs",
            from: "    if let Some(abs) = input.edited_path()\n        && role == \"worker\"",
            to: "    if let Some(abs) = input.edited_path()\n        && role == \"coordinator\"",
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
        "gate: a batch green goes on covering its bead after main moves, because `contains main` is asked of the main the run was recorded over",
        Mutation {
            // Ask about the main of this instant again, which is air-9ij exactly: a green that
            // contained main when it ran is silently disqualified the moment anybody writes to
            // main, and because cover() filters on it before the predates branch, the refusal
            // names no batch at all. The pre-v19 fallback is untouched, so the mutation
            // reaches the recorded-main question alone.
            file: "crates/cli/src/cmd/batch.rs",
            from: "            let against = g.main_sha.clone().unwrap_or_else(|| \"main\".to_string());",
            to: "            let against = \"main\".to_string();",
            also_red: &[],
        },
    ),
    (
        "gate: a bead whose every commit is already in main closes on the landing that put it there, and only on one that named it and that main still contains",
        Mutation {
            // Stop asking whether main still contains the merge, so a rewound or abandoned
            // landing would close a bead whose work is no longer anywhere. The bead-naming and
            // landed() conditions are untouched: this reaches the containment question alone,
            // which is the one that makes the row evidence rather than a memory.
            file: "crates/cli/src/cmd/batch.rs",
            from: "            && git::is_ancestor(repo, &merge, \"main\").unwrap_or(false))",
            to: "            && true)",
            also_red: &[],
        },
    ),
    (
        "gate: a digest git does not track is not proof, and the refusal names the untrailered commit that fixes it",
        Mutation {
            // Count an untracked file as tracked, which is the gate exactly as it stood: the
            // directory is read and git is never asked. The Missing case and the refusal text
            // are untouched, so the mutation reaches the tracked rule alone.
            file: "crates/cli/src/cmd/handover.rs",
            from: "        if tracked.contains(&e.file_name().to_string_lossy().to_string()) {",
            to: "        if true {",
            also_red: &[],
        },
    ),
    (
        "docs: every flag and condition kind the README and the rules name still exists",
        Mutation {
            // Check the command word and stop, which is what air-w91's check already does
            // over sources and what this bead exists because of: measured against the README
            // at c64175b, that catches NONE of the three drifts. The retired-kind half and
            // the declared-absence half are untouched, so the mutation reaches the flag rule
            // alone and the probe's `air inbox --owner` fixture is the half that falls.
            //
            // Anchored on a line carrying quotes on purpose: a mutation whose target is in
            // this file is duplicated by its own `from:` literal unless the text needs
            // escaping. The first attempt anchored on `for f in flags {`, appeared twice, and
            // was refused by the anchor probe air-g7e added — the third real thing that probe
            // has caught, and the first that was mine.
            file: "crates/cli/src/cmd/selftest.rs",
            from: "                return Err(format!(\"`air {}` has no --{f}\", path.join(\" \")));",
            to: "                let _ = f;",
            also_red: &[],
        },
    ),
    (
        "make: the verify target runs adopter-check and selftest, and release runs release-check before verify",
        Mutation {
            // Read the whole file instead of one target's recipe, which is the grep this probe
            // exists to be better than. The real Makefile still contains every command name
            // somewhere, so the RED half stays green and only the decoy half falls — the decoy
            // being a file where the names appear in a comment and in another target.
            file: "crates/cli/src/cmd/selftest.rs",
            from: "            if line.starts_with(&format!(\"{target}:\")) {",
            to: "            if true || line.starts_with(&format!(\"{target}:\")) {",
            also_red: &[],
        },
    ),
    (
        "batch: a landing records exactly the branches its batch merged, once each, and never one already in main",
        Mutation {
            // Stop excluding a worker whose branch has reached main. The batch it merged is
            // unchanged, so the RED half still names the two it merged; what falls is the
            // green half, where w1 has landed and would be re-listed on every landing after
            // its own. One condition, and the wrong one to lose quietly: the members row is
            // what a red batch is reported by.
            //
            // air-vsvt re-anchored this twice. First onto the MERGE BASE rather than the
            // worktree's head; now onto the sha filter, because the reopened bead moved the
            // sha source from the live worktrees to the batch commit's own parents, and the
            // line the previous anchor named no longer exists. The condition being neutralised
            // is the same one throughout — "already in main" — and it is still the only
            // condition this mutation touches. Anchor as rustfmt leaves it, per air-gei.
            file: "crates/cli/src/cmd/batch.rs",
            from: ".filter(|sha| !git::is_ancestor(repo, sha, tip).unwrap_or(false))",
            to: ".filter(|_sha| true)",
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
            // air-e21v, from a `--prove` run: air-jsz's probe runs the real command against a
            // repo whose leak is upper-case, so it reads this rule too. Undeclared it made this
            // mutation VACUOUS — the mutation was fine and the DECLARATION was stale.
            also_red: &[
                "privacy: adopter-check refuses a leak when run from a worktree, and refuses a repo that declares an adopter with no names instead of skipping",
            ],
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

    // air-e21v: the UNMUTATED child must parse before a single mutation is applied. It did not
    // once, and `--prove` spent its whole run reporting every mutation BROKEN — 82 rebuilds to
    // learn nothing, and the mutation evidence this repo leans on was dead in the meantime.
    // One child run against 82 is a rounding error on this command and nothing at all on
    // `make verify`, which is why the check lives here rather than in the suite.
    match build_and_run(repo) {
        Ok(v) if v.is_empty() => {
            eprintln!("air: selftest --prove: the baseline run reported no probes");
            return 2;
        }
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "air: selftest --prove stopped before applying any mutation: {e}. Every \
                 mutation would have reported BROKEN and proved nothing."
            );
            return 2;
        }
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

/// The lease-needed gate (`hook::lease_gate`): `air lease` knew who held `runtime` and not
/// which command needed it, so an adopter wrote a ~1,500-line guard that read a second store.
/// Red: w1 runs `cd app && make api` while w2 holds `runtime`; enforced, it is refused naming
/// the holder and `air lease take runtime`; advisory, it is allowed with the same words. Green:
/// w1 holding the lease runs it; the owner runs it; a quoted mention inside a `bd create`
/// description, a heredoc body, and `git grep adb` are not commands; `make test` matches nothing.
fn probe_lease_needed_gate() -> Probe {
    use crate::cmd::hook::lease_gate;
    use air_hooks::HookOutcome;
    use air_ledger::leases::{Holder, Lease};
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let decl = crate::cmd::lease::declared_from(
            &serde_json::json!({"runtime": ["make api*", "Bash(adb *)"]}),
        );
        let healthy = |_: &Lease| None;
        let take = |w: &str| {
            let h = Holder {
                worker: w,
                ..Holder::default()
            };
            l.lease_take("runtime", &h, "api", "t0", healthy)
                .map_err(|e| e.to_string())
        };
        take("w2")?;
        let gate = |w: &str, role: &str, cmd: &str, enforce: bool| {
            lease_gate(&l, w, role, &decl, cmd, enforce, healthy).map(|d| d.map(|d| d.outcome))
        };
        let refused = matches!(
            gate("w1", "worker", "cd app && make api", true)?,
            Some(HookOutcome::Block { reason })
                if reason.contains("w2") && reason.contains("air lease take runtime")
        );
        let advised = matches!(
            gate("main", "coordinator", "timeout 60 adb", false)?,
            Some(HookOutcome::Allow { context: Some(c) }) if c.contains("w2")
        );
        let quiet = |w: &str, role: &str, cmd: &str| -> Result<bool, String> {
            Ok(!matches!(
                gate(w, role, cmd, true)?,
                Some(HookOutcome::Block { .. })
            ))
        };
        let owner = quiet("main", "owner", "make api")?;
        let not_run = quiet(
            "w1",
            "worker",
            "bd create -d \"then make api && adb shell\"",
        )? && quiet(
            "w1",
            "worker",
            "git commit -F- <<EOF\nadb shell\nEOF\ngit log",
        )? && quiet("w1", "worker", "git grep -n adb && make test")?;
        l.lease_break("runtime").map_err(|e| e.to_string())?;
        take("w1")?;
        let held = quiet("w1", "worker", "FOO=1 make api-dev | tee log")?;
        Ok((refused && advised, owner && not_run && held))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "lease: a declared command from a session without the lease is refused (advised unenforced)",
        red_fires: red,
        green_passes: green,
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
        probe_lease_needed_gate(),
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
        probe_batch_ready_wants_a_precheck_where_declared(),
        probe_a_precheck_green_is_never_a_verify_green(),
        probe_batch_cut_drops_by_the_order_rule(),
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
        probe_batch_green_survives_main_moving_under_it(),
        probe_epic_with_no_open_children_is_named(),
        probe_ancestor_deadlock_is_named(),
        probe_a_red_runs_output_is_kept(),
        probe_stop_never_advises_a_lane_worker_to_merge_or_verify(),
        probe_a_prefix_claim_is_recorded_and_survives_the_reconcile(),
        probe_the_build_reaches_a_reader(),
        probe_batch_members_are_the_shas_the_batch_took(),
        probe_a_discharged_clause_names_its_lookup(),
        probe_a_journal_only_branch_needs_no_bead(),
        probe_a_landed_bead_closes_on_its_landing(),
        probe_red_batch_is_reported_by_member_and_lands_nothing(),
        probe_install_lag_is_named(),
        probe_no_session_reads_stuck(),
        probe_hook_reads_from_the_worktree_root(),
        probe_acceptance_budget_scales_with_ids(),
        probe_contradicts_names_only_the_refuted(),
        probe_unresolvable_path_is_unreadable_not_refuted(),
        probe_land_names_a_branch(),
        probe_no_flow_dependent_prescription_when_a_clause_is_undischarged(),
        probe_closed_bead_not_landed_is_named(),
        probe_capture_records_where_it_was_written(),
        probe_capture_takes_a_file_whole(),
        probe_close_with_proof_sequence(),
        probe_verify_in_flight(),
        probe_landing_state(),
        probe_land_role_is_the_launchers(),
        probe_landable_pushes_once_per_branch(),
        probe_nothing_unverified_reaches_main(),
        probe_air_runs_no_conflicting_merge(),
        probe_idle_without_claim_counts_claimable_only(),
        probe_idle_without_claim_silent_while_verifying(),
        probe_idle_without_claim_silent_while_its_tree_is_read(),
        probe_handover_names_a_red_batch_you_are_in(),
        probe_epic_count_carries_no_instruction(),
        probe_doctor_names_the_binary_against_the_checkout(),
        probe_overlap_names_only_holders_that_can_collide(),
        probe_refusal_names_the_command_it_refused(),
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
        probe_a_batch_records_exactly_the_branches_it_merged(),
        probe_the_gate_runs_what_the_makefile_says(),
        probe_docs_name_real_flags_and_kinds(),
        probe_an_untracked_digest_is_not_proof(),
        probe_the_peer_warning_dates_the_entry(),
        probe_the_refusal_says_which_not_green_state_it_is(),
        probe_landable_does_not_depend_on_which_worktree_asked(),
        probe_a_batch_records_the_shas_it_took_and_never_drops_one(),
        probe_the_assignee_refusal_says_whether_anyone_holds_it(),
        probe_close_takes_a_reason_file_whole(),
        probe_record_refuses_an_unresolved_merge_but_not_a_dirty_tree(),
        probe_scaffolded_verify_fails_until_edited(),
        probe_reclaim_churn_reads_the_owner_gated_population(),
        probe_a_stopped_session_is_recorded_and_says_whether_it_recovers(),
        probe_adopter_check_refuses_from_a_worktree_and_when_it_has_no_list(),
        probe_a_standing_red_batch_is_not_aged_out_by_later_runs(),
        probe_a_row_with_no_transcript_is_named_and_never_announced(),
        probe_selftest_json_is_only_the_array(),
        probe_no_flow_dependent_fix_asserts_a_forbidden_repair(),
        probe_only_a_failed_handover_counts_as_an_attempt(),
        probe_the_journal_is_scaffolded_and_nothing_reads_it(),
        probe_status_json_says_why_a_branch_cannot_land(),
        probe_handover_says_what_the_landing_gate_would_say(),
        probe_a_timed_out_fast_forward_is_not_reported_as_untouched(),
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
        fix: "a green at that head; `air handover` in that worktree names what it needs"
            .to_string(),
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
    use crate::cmd::handover::{Digest, declared_bead, digest_for_bead};

    let res = (|| -> Option<(bool, bool)> {
        let root = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&root).ok()?;
        let dir = root.as_path();
        // Strict: every file here counts as written after the cutoff, so only a declaration
        // is accepted. The lenient cutoff below is the history case.
        let cut: jiff::Timestamp = "2000-01-01T00:00:00Z".parse().ok()?;
        let write = |name: &str, body: &str| std::fs::write(dir.join(name), body).ok();
        // air-ahl: this probe is about DECLARING a bead, not about tracking, so everything it
        // writes is declared tracked and the tracked rule is probed separately.
        let all_tracked = |names: &[&str]| -> std::collections::BTreeSet<String> {
            names.iter().map(|n| (*n).to_string()).collect()
        };
        // A digest for ANOTHER bead, by this worker, written now.
        write(
            "2026-08-23-beta-air-other.md",
            "---\nbead: air-other\n---\n# other\n",
        )?;
        let ours = vec!["air-agq".to_string()];

        // Red: it declares a different bead, so it is not this bead's digest, whatever its
        // name or mtime say.
        let tracked = all_tracked(&["2026-08-23-beta-air-other.md"]);
        let wrong_bead =
            digest_for_bead(dir, "beta", &ours, None, cut, &tracked) == Digest::Missing;
        // Red: a file carrying the worker's name and no declaration, written after the
        // cutoff, is not a substitute — this is the `touch` case and the substring case.
        write("2026-08-23-beta-notes.md", "# just some notes\n")?;
        let tracked = all_tracked(&["2026-08-23-beta-air-other.md", "2026-08-23-beta-notes.md"]);
        let undeclared_after_cutoff =
            digest_for_bead(dir, "beta", &ours, None, cut, &tracked) == Digest::Missing;
        let red =
            wrong_bead && undeclared_after_cutoff && declared_bead("# no front matter").is_none();

        // Green: the digest that declares this bead is accepted.
        write(
            "2026-08-23-beta-air-agq.md",
            "---\nbead: air-agq\n---\n# ours\n",
        )?;
        let tracked = all_tracked(&[
            "2026-08-23-beta-air-other.md",
            "2026-08-23-beta-notes.md",
            "2026-08-23-beta-air-agq.md",
        ]);
        let declared_ok =
            digest_for_bead(dir, "beta", &ours, None, cut, &tracked) == Digest::Tracked;

        // Green: history still passes. A digest written before the cutoff with no front
        // matter is matched the old way, so the change does not invalidate what exists.
        let old = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&old).ok()?;
        std::fs::write(old.join("2026-08-22-beta-air-old.md"), "# old\n").ok()?;
        let far_future: jiff::Timestamp = "2999-01-01T00:00:00Z".parse().ok()?;
        let fallback_ok = digest_for_bead(
            &old,
            "beta",
            &ours,
            None,
            far_future,
            &all_tracked(&["2026-08-22-beta-air-old.md"]),
        ) == Digest::Tracked;
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
/// air-45pw: `air capture` took one positional and nothing else, so a finding long enough to be
/// worth writing went through the harness's command classifier as a command line and was refused
/// for its shape. An adopter's worker shortened a finding in order to file it. Capture is the
/// intake for everything the coordinator triages, and a shortened capture looks exactly like a
/// capture, so the loss is invisible.
///
/// The assertion is byte-for-byte equality with the file, deliberately, and not "non-empty" or
/// "long enough". What this bug produces is a TRUNCATION, and a probe that checks for presence
/// passes over the exact failure — the same shape as an anchor where everything reads as fine.
/// The fixture is far longer than any sensible command line and carries its last sentence as a
/// sentinel, so a cut anywhere is a failure and not a smaller pass.
///
/// Red: both routes at once, and neither, are each refused, and each refusal names both routes —
/// the person reading it has just had a capture refused and needs to be told the other way in.
/// Green: the long file round-trips through the real `capture` into the ledger unchanged, and
/// the positional route still works beside it.
fn probe_capture_takes_a_file_whole() -> Probe {
    use crate::cmd::capture::{capture, resolve_text};
    use crate::cmd::open;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
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

        // Longer than any command line anyone would write, with interior blank lines, quotes and
        // apostrophes — the shapes that make a classifier refuse — and a sentinel last sentence.
        let mut finding = String::new();
        for i in 0..60 {
            finding.push_str(&format!(
                "Paragraph {i}: the worker's own words, with \"quotes\", a $dollar and a `tick`, \
                 running past the length at which a shell argument stops being reasonable.\n\n"
            ));
        }
        finding.push_str("SENTINEL: the last sentence, which a truncation eats first.");
        let path = dir.join("finding.md");
        std::fs::write(&path, &finding).map_err(|e| e.to_string())?;

        // Red: neither route, and both at once. Each refusal must name BOTH ways in.
        let names_both = |e: &str| e.contains("--file") && e.contains("text");
        let neither = resolve_text(None, None);
        let both = resolve_text(Some("a line"), Some(&path));
        let refusals_are_useful =
            matches!(&neither, Err(e) if names_both(e)) && matches!(&both, Err(e) if names_both(e));

        // Green: the real path. `capture` writes to the repo's ledger; read it back out.
        let filed = capture(&dir, None, Some(&path), "coordinator", true) == 0;
        let (ledger, _) = open(&dir)?;
        let items = ledger.inbox().map_err(|e| e.to_string())?;
        // Byte for byte against the file, not "non-empty" and not "long": a truncation is what
        // the bug produces and it is what this must catch.
        let whole = items.len() == 1
            && items.first().is_some_and(|c| {
                c.text == finding
                    && c.text.len() == finding.len()
                    && c.text.ends_with("a truncation eats first.")
            });

        // The positional still works: the file route is an addition, not a replacement, because
        // most captures are one-liners.
        let line_ok = capture(&dir, Some("  a one-liner  "), None, "coordinator", true) == 0;
        let both_filed = ledger
            .inbox()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|c| c.text == "a one-liner");

        std::fs::remove_dir_all(&dir).ok();
        Ok((refusals_are_useful, filed && whole && line_ok && both_filed))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "capture: --file files the finding whole, byte for byte, and passing both routes or neither is refused naming both",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-jy99: the `landed-not-closed` line ended "either reopen it or file what is left", and an
/// adopter's CLAUDE.md says "closed is closed — never reopen". Air was instructing their
/// coordinator to do what their own rules forbid, on a bead that was correctly closed.
///
/// This is air-155w's ruling at a second surface: a flow-dependent fix states a CONDITION, not a
/// command, because whose job a thing is — and whether it is allowed at all — is the repo's flow
/// to say and Air reads no repo's flow. What makes it worth its own probe rather than one more
/// assertion on the sentence's existing one is the failure air-155w recorded: air-avj fixed the
/// Stop hook and left the refusal asserting the forbidden command, so the adopter's worker read
/// it from the refusal instead. Following such a line WORKS, so nothing ever contradicts it.
/// Hence both renderers, checked together: the same clause reached the reader twice.
///
/// Red: the prescription restored anywhere it is rendered is caught. Green: neither shipped
/// string names the action, both still say what must become true, and the two things air-k6uh
/// established — the lookup framing, and "it is done elsewhere" as the first option — survive.
fn probe_no_flow_dependent_prescription_when_a_clause_is_undischarged() -> Probe {
    use crate::cmd::status::{Snapshot, Thresholds, attention, kinds};
    use air_ledger::landings::{Landing, OpenBead};

    let res = (|| -> Result<(bool, bool), String> {
        // The real attention sentence, rendered from a real row rather than quoted.
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
            beads: vec!["zz-1".into()],
            open_beads: vec![OpenBead {
                bead: "zz-1".into(),
                why: "docs/absent.md says it.".into(),
                refuted: true,
                contradicted: "docs/absent.md says it.".into(),
            }],
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
        let line = att
            .iter()
            .find(|a| a.kind == kinds::LANDED_NOT_CLOSED)
            .map(|a| a.detail.clone())
            .ok_or("no landed-not-closed line")?;

        // `air land`'s summary is the SAME clause rendered a second time. air-155w's defect
        // survived its first fix because one renderer was corrected and the other was not, so
        // this probe fails unless both are clean.
        let land_summary = crate::cmd::land::REFUTED_SUMMARY;

        // No attention sentence of any kind prescribes it either: a second condition rendering
        // this advice is the way it would come back.
        let nothing_prescribes = att.iter().all(|a| !a.detail.contains("reopen"));

        let red =
            !line.contains("reopen") && !land_summary.contains("reopen") && nothing_prescribes;

        // Saying less is only right if it still says what must be established. The line names
        // the condition and hands the decision back, and air-k6uh's two survive.
        let green = line.contains("untracked")
            && line.contains("this repo's flow to say")
            && line.contains("it is done elsewhere")
            && line.contains("NOT a contradiction")
            && land_summary.contains("nothing left over is untracked")
            && !land_summary.contains("CONTRADICTS");
        Ok((red, green))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "status/land: an undischarged clause names the condition and no flow-dependent action, in both renderers",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-gazh: a bead closed with proof whose commits are in no tree but its author's worktree.
/// An adopter had six at once, derived independently by their coordinator and their worker from
/// different joins — same six — and there is no error anywhere in how they got there: closed on
/// a batch green, main moved, the batch stopped containing main, later cuts redded or were
/// killed. Bead closed, branch green, tree clean. `landable` needs a branch containing main so
/// it goes quiet the moment main moves; `landed-not-closed` needs a landing. Nothing joined
/// closed ∩ NOT landed.
///
/// **Both halves are pinned deliberately**, per the bead: the value of this line is that it
/// fires rarely, so a version that fires on a landed bead or on an open one is worse than
/// nothing. A landed bead cannot reach the join at all — `select` drops a branch already in
/// main — so the half this probe has to prove is the OPEN one, which reaches the join and must
/// be rejected there.
///
/// Red: a closed bead on a branch that has not landed is named, with its worker and head.
/// Green: an open bead on the same unlanded branch produces nothing, and neither does a closed
/// bead whose claim was released for any other reason — the join reads the `closed` release
/// reason and not merely "the claim ended".
fn probe_closed_bead_not_landed_is_named() -> Probe {
    use crate::cmd::status::{Snapshot, Thresholds, attention, kinds};

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        // Three beads on one worker's unlanded branch, released three ways. Only the one the
        // reconcile marked `closed` is this condition's subject.
        for (bead, reason) in [
            ("zz-closed", "closed"),
            ("zz-landed", "landed"),
            ("zz-abandoned", "abandoned"),
        ] {
            l.record_claim(bead, "alpha", &[], "t0")
                .map_err(|e| e.to_string())?;
            l.release_claim(bead, "alpha", reason, "t1")
                .map_err(|e| e.to_string())?;
        }
        // ...and one still open, which is the ordinary state of work in flight.
        l.record_claim("zz-open", "alpha", &[], "t0")
            .map_err(|e| e.to_string())?;

        let closed: std::collections::BTreeSet<String> = l
            .closed_claims()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|c| c.bead)
            .collect();
        // The ledger half: `closed` alone, not "the claim ended". Released-as-landed and
        // released-as-abandoned are both claim endings and neither is this.
        let reads_the_reason = closed.len() == 1 && closed.contains("zz-closed");

        // The join half, through the real condition. A `Snapshot` with these beads on an
        // unlanded branch is what `select` produces for a branch behind main.
        let snap = Snapshot {
            closed_not_landed: vec![crate::cmd::status::ClosedNotLanded {
                bead: "zz-closed".into(),
                worker: "alpha".into(),
                head: "abcdef1234".into(),
                blocked: Some("refused: branch does not contain main".into()),
            }],
            ..Default::default()
        };
        let att = attention(&snap, "2026-09-07T00:00:00Z", Thresholds::default());
        let named: Vec<_> = att
            .iter()
            .filter(|a| a.kind == kinds::CLOSED_NOT_LANDED)
            .collect();
        let names_it = named.len() == 1
            && named.first().is_some_and(|a| {
                a.worker == "alpha"
                    && a.detail.contains("zz-closed")
                    && a.detail.contains("abcdef12")
                    && a.detail.contains("does not contain main")
            });

        // An empty join says nothing at all: the quiet case is the common one and must stay
        // silent, or the line stops being worth reading when it does fire.
        let quiet = attention(
            &Snapshot::default(),
            "2026-09-07T00:00:00Z",
            Thresholds::default(),
        )
        .iter()
        .all(|a| a.kind != kinds::CLOSED_NOT_LANDED);

        Ok((reads_the_reason && names_it, quiet))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "status: a closed bead whose commits reached no tree but its author's worktree is named, and a claim that merely ended is not",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-6dj4: a capture records WHEN it was written but not WHERE, so a fact in its body travels
/// without its sha. The time is on the row and the subject is in the body, and the body is what
/// gets quoted into a bead or a message — "the batch is red" arrives elsewhere with no way to
/// say which batch. The reported cause ("a capture carries no timestamp") was false; `air inbox`
/// renders `captured_at`. The real defect is that the timestamp is DETACHABLE, and that
/// correction is what picked this fix over rendering an age.
///
/// **The half that can go wrong quietly is the absence**, which is why it is half this probe. A
/// capture written where there is no head must record that Air looked and found none, with the
/// reason — not an empty sha, which would render as a commit nobody can find while every other
/// check passed. And a row from before this column must say nothing at all, because Air observed
/// nothing there and "no head" is a claim somebody has to have made.
///
/// Red: the stored sha is the worktree's real HEAD, compared against `git rev-parse` rather than
/// a constructed value; and the three states render three ways.
/// Green: no head is recorded WITH a reason rather than as an empty string, and a pre-v21 row
/// (both columns NULL) stays silent instead of borrowing the absence's wording.
fn probe_capture_records_where_it_was_written() -> Probe {
    use crate::cmd::open;
    use air_ledger::captures::Head;

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
        // The answer this must match: git's own, not a value this probe built.
        let real_head = g(&["rev-parse", "HEAD"])?;

        // The real path, through the CLI command that does the lookup.
        let code =
            crate::cmd::capture::capture(&dir, Some("the batch is red"), None, "coordinator", true);
        let (ledger, _) = open(&dir)?;
        let items = ledger.inbox().map_err(|e| e.to_string())?;
        let stored_is_the_head = code == 0
            && items.len() == 1
            && items
                .first()
                .is_some_and(|c| c.head == Some(Head::At(real_head.clone())));

        // Three states, three renderings, none of them each other.
        let at = crate::cmd::capture::where_written(Some(&Head::At(real_head.clone())));
        let absent = crate::cmd::capture::where_written(Some(&Head::Absent("no repo".into())));
        let never = crate::cmd::capture::where_written(None);
        let three_ways = at.contains(real_head.get(..8).unwrap_or(&real_head))
            && absent.contains("no head")
            && absent.contains("no repo")
            && never.is_empty()
            && at != absent;

        // A directory that is no repo: the absence is RECORDED with its reason. An empty sha
        // here would render as a commit nobody can look up and nothing would raise it.
        let bare = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&bare).map_err(|e| e.to_string())?;
        let l2 = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let none_here = crate::cmd::capture::head_now(&bare);
        let names_the_absence =
            matches!(&none_here, Some(Head::Absent(why)) if !why.trim().is_empty());
        l2.capture("c1", "probe", None, "text", "t0", none_here.as_ref())
            .map_err(|e| e.to_string())?;
        // ...and a row written with nothing recorded stays `None`, not `Absent("")`.
        l2.capture("c2", "probe", None, "text", "t0", None)
            .map_err(|e| e.to_string())?;
        let rows = l2.inbox().map_err(|e| e.to_string())?;
        let kept_apart = rows
            .iter()
            .any(|c| c.id == "c1" && matches!(&c.head, Some(Head::Absent(w)) if !w.is_empty()))
            && rows.iter().any(|c| c.id == "c2" && c.head.is_none());

        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&bare).ok();
        Ok((
            stored_is_the_head && three_ways,
            names_the_absence && kept_apart,
        ))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "capture: the row records the worktree's real head, and a capture written where there is no head says so with a reason rather than an empty sha",
        red_fires: res.0,
        green_passes: res.1,
    }
}

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
                main_sha: None,
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
        ".claude/skills/writing-docs/references/registers.md".to_string(),
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
        vec![".claude/skills/writing-docs/references/registers.md names the rule.".into()],
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
            && open.first().is_some_and(|o| {
                o.bead == "zz-2"
                    && o.why
                        .contains(".claude/skills/writing-docs/references/registers.md")
            });
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
        ".claude/skills/writing-docs/references/registers.md".to_string(),
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
                    // air-k6uh: the split air-ppf established is unchanged — only the
                    // clauses whose named file the merge did not change are here. What
                    // changed is the CLAIM: the sentence reports the lookup and says in as
                    // many words that it is not a contradiction, because six of nine such
                    // firings on 2026-09-06 were clauses that held.
                    && a.detail.contains("naming a file this merge did not change")
                    && a.detail.contains("\"docs/absent.md says it.\"")
                    && a.detail.contains("NOT a contradiction")
                    && !a.detail.contains("CONTRADICTS")
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
        name: "status: landed-not-closed names only the clauses whose file the merge did not change, claims no contradiction, and stays silent about a bead Air merely could not read",
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
        bead: Some(bead.into()),
        worker: worker.into(),
        head: format!("{worker}0000"),
        minutes,
        command: match blocked {
            None => format!("air land --worker {worker}"),
            Some(_) => crate::cmd::land::remerge_command(),
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
        bead: Some(bead.into()),
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
        let mut b: Vec<&str> = v.iter().filter_map(|l| l.bead.as_deref()).collect();
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

    let red = may_close("worker").is_err();
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
        Ok(may_close("coordinator").is_ok() && one_process && released.len() == ids.len())
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
    let red = may_land("worker").is_err()
        && sites
            .iter()
            .all(|s| check(s, &ok()).err().is_some_and(readable))
        && refusals
            .iter()
            .all(|f| check(&here(), f).err().is_some_and(readable));
    let green = may_land("coordinator").is_ok()
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

/// air-t6ap: `idle-without-claim` is silent for a worker whose own verify is running.
///
/// An adopter's verify lane was told to claim one of 58 beads **945 seconds into a batch
/// verify**, while `air status` printed `verify in flight: w4 started 945s ago` three lines
/// above. A lane holds no bead while it batches — roles.md says so in those words (air-80x.6)
/// — so the condition fired on a documented state, and the remedy it named is the failure it
/// exists to prevent: a lane that claims a bead mid-batch cannot cut the batch, and three
/// workers were waiting on that run. Their coordinator checked the run was alive and did not
/// prompt. An unattended one would have interrupted it at minute fifteen.
///
/// The discriminating fact was never missing. `verifies_in_flight` is a field on the same
/// snapshot this predicate reads, populated by the same `gather`, printed by the same command.
///
/// Red: an in-flight verify for this worker silences the condition. Green: the same worker with
/// no verify running is still reported, and a verify belonging to somebody else does not
/// silence it — so the fix cannot be a blanket suppression.
///
/// The precheck ruling of 2026-09-25 widened red to a `precheck` in flight: an adopter's coordinator nudged a worker
/// as idle 400 s into a precheck Air could not see. It silences the condition and the status
/// line's "idle, no claim" alike, and the in-flight line names the kind. The mutation that made
/// that half red, seen: the status line's `idle_no_claim` without `!verify_running(..)`.
fn probe_idle_without_claim_silent_while_verifying() -> Probe {
    use crate::cmd::status::{Snapshot, Thresholds, attention, render_for_probe};

    // Whoever the in-flight row names; None for no row at all.
    let at_kind = |running: Option<&str>, kind: air_ledger::verify::Kind| Snapshot {
        at: "2026-09-06T12:30:00Z".into(),
        workers: vec![crate::cmd::status::WorkerView {
            worker: "w4".into(),
            role: "worker".into(),
            session: Some(crate::cmd::status::Session {
                session_id: "s".into(),
                state: "idle".into(),
                changed_at: "2026-09-06T12:00:00Z".into(),
                pid_alive: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ready_depth: Some(58),
        claimable_depth: Some(58),
        verifies_in_flight: running
            .map(|w| {
                vec![air_ledger::verify::InFlight {
                    id: "01J".into(),
                    worker: w.into(),
                    sha: "cafe1234".into(),
                    kind,
                    command: "make verify".into(),
                    pid: Some(4242),
                    started_at: "2026-09-06T12:14:15Z".into(),
                }]
            })
            .unwrap_or_default(),
        ..Default::default()
    };
    let at = |running: Option<&str>| at_kind(running, air_ledger::verify::Kind::Verify);
    let fires = |s: &Snapshot| {
        attention(s, "2026-09-06T12:30:00Z", Thresholds::default())
            .iter()
            .any(|a| a.kind == "idle-without-claim")
    };

    // The reported state, to the second: idle 30 min, 58 claimable, own verify 945 s in.
    let prechecking = at_kind(Some("w4"), air_ledger::verify::Kind::Precheck);
    let shown = render_for_probe(&prechecking);
    let red = !fires(&at(Some("w4")))
        && !fires(&prechecking)
        && shown.contains("precheck in flight: w4")
        && !shown.contains("idle, no claim");
    let green =
        // Without the run, this is an ordinary idle worker and the condition is the point.
        fires(&at(None))
        // Somebody else's verify says nothing about whether w4 can take a bead.
        && fires(&at(Some("other")));
    Probe {
        name: "attention: idle-without-claim is silent for a worker whose own verify is in flight",
        red_fires: red,
        green_passes: green,
    }
}

/// Tree readers (owner, 2026-09-25): `idle-without-claim` is silent for a worker whose tree has
/// a process that is not its session, and `air land`'s warning names a reader of main.
///
/// An adopter's worker under a lane ran an unrecorded precheck; with no claim and no recorded
/// run it read as idle and the coordinator prompted a busy worker (2026-09-07). Their fix was
/// a process listing by cwd, run before calling anyone idle.
///
/// Red: a `cargo` reached through the worker's shell, cwd in its tree, silences the condition.
/// Green: the condition still fires with no reader, with only the session's own processes in
/// the tree (claude, its `air mcp`), with the reader in another worker's tree, and when the
/// lookup could not answer, so the exemption cannot be a blanket one; and the land warning
/// names the pid of a reader of main and says nothing when only a session is there.
fn probe_idle_without_claim_silent_while_its_tree_is_read() -> Probe {
    use crate::cmd::readers::{Proc, Raw, TreeReaders, group, main_warning};
    use crate::cmd::status::{Snapshot, Thresholds, attention};

    let proc_ = |pid: i64, ppid: i64, command: &str| Proc {
        pid,
        ppid,
        elapsed_secs: Some(200),
        command: command.into(),
    };
    let trees = vec![
        ("main".to_string(), "/r".to_string()),
        ("w1".to_string(), "/r/.claude/worktrees/w1".to_string()),
        ("w10".to_string(), "/r/.claude/worktrees/w10".to_string()),
    ];
    // claude (11) in w1 with its MCP server (12) and a Bash-tool shell (13); `extra` adds
    // processes with their cwds.
    let readers = |extra: &[(Proc, &str)]| {
        let mut raw = Raw {
            procs: vec![
                proc_(10, 1, "-zsh"),
                proc_(11, 10, "claude"),
                proc_(12, 11, "air"),
                proc_(13, 11, "zsh"),
            ],
            cwds: vec![
                (11, "/r/.claude/worktrees/w1".into()),
                (12, "/r/.claude/worktrees/w1".into()),
            ],
        };
        for (p, cwd) in extra {
            raw.cwds.push((p.pid, (*cwd).to_string()));
            raw.procs.push(p.clone());
        }
        group(&raw, &trees, &std::collections::BTreeSet::new(), 99_999)
    };
    let snap = |r: TreeReaders| Snapshot {
        at: "2026-09-07T12:30:00Z".into(),
        workers: vec![crate::cmd::status::WorkerView {
            worker: "w1".into(),
            role: "worker".into(),
            session: Some(crate::cmd::status::Session {
                session_id: "s".into(),
                state: "idle".into(),
                changed_at: "2026-09-07T12:00:00Z".into(),
                pid_alive: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ready_depth: Some(5),
        claimable_depth: Some(5),
        tree_readers: r,
        ..Default::default()
    };
    let fires = |r: TreeReaders| {
        attention(&snap(r), "2026-09-07T12:30:00Z", Thresholds::default())
            .iter()
            .any(|a| a.kind == "idle-without-claim")
    };
    let precheck = [(proc_(14, 13, "cargo"), "/r/.claude/worktrees/w1/crates")];

    let red = !fires(readers(&precheck));
    let green = fires(readers(&[]))
        && fires(readers(&[(
            proc_(14, 13, "cargo"),
            "/r/.claude/worktrees/w10",
        )]))
        && fires(TreeReaders::unknown("lsof could not start"))
        && main_warning(&readers(&[(proc_(20, 1, "node"), "/r/src")]), "/r")
            .is_some_and(|w| w.contains("pid 20 node"))
        && main_warning(&readers(&[(proc_(21, 11, "caffeinate"), "/r")]), "/r").is_none();
    Probe {
        name: "attention: idle-without-claim is silent for a worker whose tree has a process that is not its session, and the land warning names it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-hpp8: a member of a standing red batch can look it up, and nothing ever claims
/// non-membership.
///
/// A lane's red batch reached its members by the lane remembering to message each of them.
/// Twice in one night in the adopter's fleet somebody was left off — the second time after the
/// lane had amended its practice to "every member and the coordinator" — and the omitted
/// worker spent 132 s running a suite against a hypothesis the batch log had already refuted.
/// `red_batch_standing` knew the answer the whole time and `air handover` did not say it.
///
/// Red: a member is told, with the batch sha, the lane, and where the lane's output is. Green:
/// a worker not among recorded members is told nothing, a batch with an EMPTY members list is
/// told nothing either, and in neither case does the output contain a claim that the worker
/// was not in a batch — the two silences have different reasons and the same output, which is
/// the ruling (reading A) and is air-sdjo's to change.
fn probe_handover_names_a_red_batch_you_are_in() -> Probe {
    use crate::cmd::batch::RedBatch;
    use crate::cmd::handover::red_batch_line;
    use air_ledger::landings::Member;

    let batch = |members: Vec<(&str, &str)>, log: Option<&str>| RedBatch {
        sha: "deadbeefcafe".into(),
        worker: "lane".into(),
        at: "2026-09-06T22:10:00Z".into(),
        members: members
            .into_iter()
            .map(|(w, sha)| Member {
                worker: w.into(),
                sha: sha.into(),
            })
            .collect(),
        log_path: log.map(str::to_string),
    };

    let full = batch(
        vec![("alpha", "aaaaaaaabbbb"), ("beta", "bbbbbbbbcccc")],
        Some(".air/runs/01J.log"),
    );
    let line = red_batch_line(Some(&full), "beta");
    let red = line.as_deref().is_some_and(|l| {
        l.contains("RED batch deadbeef")
            && l.contains("cut by lane")
            && l.contains("2026-09-06T22:10:00Z")
            // The member's OWN sha, so they know which of their commits was in it.
            && l.contains("bbbbbbbb")
            // The evidence, not just the verdict: this is what the omitted worker had to go
            // and find in another worktree.
            && l.contains(".air/runs/01J.log")
    });

    // A batch that kept no output still answers the question it is asked.
    let no_log = red_batch_line(Some(&batch(vec![("beta", "bbbbbbbbcccc")], None)), "beta");
    let green =
        // Recorded members that do not include this worker: nothing extra.
        red_batch_line(Some(&full), "gamma").is_none()
        // An empty members list: the state where Air does not KNOW who was in it. Silent,
        // and the silence is the same one a genuine non-member gets, on purpose.
        && red_batch_line(Some(&batch(vec![], None)), "beta").is_none()
        // No standing batch at all.
        && red_batch_line(None, "beta").is_none()
        // Nothing anywhere asserts the negative, which is the answer that would be WRONG
        // whenever the members list was dropped.
        && !line.as_deref().unwrap_or_default().contains("not in")
        && no_log
            .as_deref()
            .is_some_and(|l| l.contains("kept no output"));
    Probe {
        name: "handover: a member of a standing red batch is told so, and a non-member is never told it was not",
        red_fires: red,
        green_passes: green,
    }
}

/// air-3vkg: the epic COUNT carries no instruction, and the line that can tell carries it.
///
/// `air status` said "N epic(s) to decompose, not claimable" where N is `epic_depth` — every
/// epic in the ready set, decomposed or not. An adopter's coordinator audited all six of
/// theirs on the strength of that phrase and **every one was already at its correct
/// frontier**, one of them fully cut with thirteen claimable children. The honest number is
/// `epics_to_decompose` (air-84u: no OPEN child), and it prints two lines below as `epic ready
/// to decompose: <id>` — so Air computed the right answer, printed it directly underneath, and
/// attached the action-word to the other one. Third time in a day that a line's discriminating
/// fact was on the same struct.
///
/// **The count stays on `epic_depth` on purpose**, and this probe pins that rather than
/// leaving it to the comment. `epic_depth` is cached and restored when bd is slow;
/// `epics_to_decompose` needs a `children` call per epic and is `None` there. Re-pointing the
/// count would make the line silent exactly when bd is slow, which is when a coordinator is
/// most likely to be reading a cached status.
///
/// Red: neither rendering attaches an instruction to the count — the fresh path and the cache
/// path both. Green: the named line below names the undecomposed epic and NOT the cut one,
/// and the cache path still reports the count while marking itself cached, so "asked, none to
/// decompose" and "not asked" stay distinguishable on the same line.
fn probe_epic_count_carries_no_instruction() -> Probe {
    use crate::cmd::status::{EpicToDecompose, Snapshot, render_for_probe};

    // Two epics in ready. One is fully cut (thirteen open children, so not this coordinator's
    // to decompose); one has nothing open under it and is.
    let fresh = render_for_probe(&Snapshot {
        ready_depth: Some(5),
        claimable_depth: Some(3),
        epic_depth: Some(2),
        epics_to_decompose: Some(vec![EpicToDecompose {
            epic: "zz-open".into(),
            closed_children: 4,
        }]),
        ..Default::default()
    });
    // bd was slow: the count survives from the cache, the per-epic children calls did not run.
    let cached = render_for_probe(&Snapshot {
        ready_depth: Some(5),
        claimable_depth: Some(3),
        epic_depth: Some(2),
        epics_to_decompose: None,
        bd_source: "cache",
        ..Default::default()
    });

    let red = !fresh.contains("epic(s) to decompose") && !cached.contains("epic(s) to decompose");
    let green =
        // The count is `epic_depth` — 2, not the 1 that is to decompose. Asserted without
        // the surrounding wording, so the declared mutation takes out the red half only.
        fresh.contains("2 epic(s)")
        // The instruction is on the line that can tell, and it names only the epic it is true of.
        && fresh.contains("epic ready to decompose: zz-open (0 open children, 4 closed)")
        && !fresh.contains("zz-cut")
        // The cache path still answers, and says which path it is, so silence there is not
        // read as "nothing to decompose".
        && cached.contains("2 epic(s)")
        && cached.contains("(cached; bd not called this tick)")
        && !cached.contains("epic ready to decompose");
    Probe {
        name: "status: the epic count carries no instruction, and the line that knows which epics are undecomposed carries it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-ilh4: `air doctor` names the running binary against the checkout it is run in, and does
/// it only where that comparison means something.
///
/// The round of 2026-09-07 ran `air` 0.2.19 against a checkout that had reached 0.3.5 and
/// quoted both interchangeably for twelve hours. Every number read from `air status`, `air
/// audit` or the ledger described the frozen binary; every digest's probe count described the
/// tree. It cost a red verify with a null `log_path` that could not be diagnosed — air-5ik,
/// which writes run logs, landed after that binary was cut — and it nearly cost a filed defect
/// reading "the runlog mechanism fires 1 in 13", where twelve of the thirteen were recorded by
/// a binary that has no runlog. `air doctor` compared the install RECORD against the running
/// binary and never the running binary against main.
///
/// Red: in Air's own checkout at a different version, the line names both and says which side
/// each kind of number comes from. Green: **a checkout that is not Air's gets nothing**, which
/// is the half that matters — in an adopting repo `main` is their code and a line here would
/// fire everywhere and be true nowhere — and neither does a checkout that agrees with the
/// binary.
fn probe_doctor_names_the_binary_against_the_checkout() -> Probe {
    use crate::cmd::doctor::{air_checkout_version, build_gap, build_gap_line};

    let make = |dir: &std::path::Path, crate_name: &str, version: &str| -> std::io::Result<()> {
        std::fs::create_dir_all(dir.join("crates/cli"))?;
        std::fs::write(
            dir.join("crates/cli/Cargo.toml"),
            format!("[package]\nname = \"{crate_name}\"\nversion.workspace = true\n"),
        )?;
        std::fs::write(
            dir.join("Cargo.toml"),
            format!(
                "[workspace]\nmembers = [\"crates/cli\"]\n\n\
                 [workspace.package]\nversion = \"{version}\"\nedition = \"2024\"\n"
            ),
        )
    };

    let res = (|| -> std::io::Result<(bool, bool)> {
        let tmp = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&tmp)?;
        let (ours, theirs, same) = (tmp.join("air"), tmp.join("adopter"), tmp.join("same"));
        make(&ours, "air", "0.3.5")?;
        // An adopting repo that happens to have a `crates/cli` of its own. This is the fixture
        // the guard exists for: without it the version read succeeds and the line fires here.
        make(&theirs, "their-cli", "9.9.9")?;
        make(&same, "air", "0.2.19")?;

        let g = build_gap(&ours, "0.2.19");
        let line = g.as_ref().map(build_gap_line).unwrap_or_default();
        let red = g.is_some()
            && line.contains("running air 0.2.19")
            && line.contains("builds 0.3.5")
            // Which side each kind of number comes from, said rather than left to be derived.
            && line.contains("`air status`")
            && line.contains("make verify");

        let green =
            // Not Air's checkout: silent, even though the version is right there to read.
            build_gap(&theirs, "0.2.19").is_none()
            && air_checkout_version(&theirs).is_none()
            // Air's checkout, agreeing: nothing to disambiguate, so nothing said.
            && build_gap(&same, "0.2.19").is_none()
            // The version itself still reads, so the silence above is the comparison and not
            // a failure to parse.
            && air_checkout_version(&same).as_deref() == Some("0.2.19")
            && air_checkout_version(&ours).as_deref() == Some("0.3.5")
            // No Cargo.toml at all — an installed repo, not a checkout.
            && build_gap(&tmp, "0.2.19").is_none();
        let _ = std::fs::remove_dir_all(&tmp);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(|e| blocked(e.to_string()));
    Probe {
        name: "doctor: the running binary is named against the checkout it runs in, and only where that comparison means something",
        red_fires: red,
        green_passes: green,
    }
}

/// air-pwvk: the overlap line names only holders that can collide, so six clean ones produce
/// no line at all.
///
/// The predicate was "a row exists", so on a long-lived shared file it named everyone who had
/// ever touched it. Measured in this repo at the time: `CLAUDE.md` held by six workers, every
/// one `clean now`, true positives zero; `crates/cli/src/cmd/status.rs` held by TEN, of which
/// exactly one was live. An adopter saw the same shape reach a worker three times in a night
/// with no true positive, and their reader wrote *"I checked with air holdings all three
/// times; the fourth time I would not have."*
///
/// **The failure this removes is a line nobody reads**, so a probe asserting the line EXISTS
/// would pass straight over it — which is why the red half is the silence.
///
/// Red: six journaled-only holders collide with nobody. Green: two live holders are still
/// reported and are still told apart, `committed` counts as live because unlanded commits
/// really do overlap, and a single live holder among many clean ones is not an overlap.
fn probe_overlap_names_only_holders_that_can_collide() -> Probe {
    use crate::cmd::holdings::Holding;
    use crate::cmd::status::colliding;

    let h = |worker: &str, uncommitted: bool, committed: bool, journaled: bool| Holding {
        worker: worker.into(),
        uncommitted,
        committed,
        journaled,
        last_edit: Some("2026-08-22T00:00:00Z".into()),
        ..Default::default()
    };

    // The reported state: six holders, every one journaled-only and clean.
    let clean: Vec<Holding> = ["alpha", "diligence", "landing", "gate", "launch", "verify"]
        .iter()
        .map(|w| h(w, false, false, true))
        .collect();
    // Both halves of the suppression, so the declared mutation takes this half and only this
    // half: nothing survives from six clean holders, and from a mixed set only the live ones do.
    let red_clean = colliding(&clean).is_empty();
    // The six clean ones plus a single live editor, built rather than sliced.
    let one_live = {
        let mut v = clean.clone();
        v.push(h("ledger", true, false, true));
        v
    };

    // One dirty, one with unlanded commits, four remembered.
    let mut mixed = clean.clone();
    mixed.push(h("ledger", true, false, true));
    mixed.push(h("alerts", false, true, true));
    let live = colliding(&mixed);
    let names: Vec<&str> = live.iter().map(|x| x.worker.as_str()).collect();

    // "Only they do" belongs to the red half: it is an assertion about what is REMOVED.
    let red = red_clean
        && names == ["ledger", "alerts"]
        // One live holder among six clean ones is not an overlap — the case measured on
        // status.rs, where the old line named all ten holders for one live editor.
        && colliding(&one_live).len() == 1;

    // The green half asserts what must SURVIVE, and every clause of it holds with the filter
    // removed as well — so the mutation below cannot take both halves and claim more than it
    // proved. The risk this guards is the opposite of the bug: a suppression that also
    // silences a real overlap.
    let green = live.iter().any(|x| x.worker == "ledger")
        && live.iter().any(|x| x.worker == "alerts")
        // A committed-only holder is live: unlanded commits overlap even with a clean tree.
        && colliding(&[h("alerts", false, true, true)]).len() == 1
        // And when there IS something to say, the line still says it.
        && crate::cmd::status::render_for_probe(&crate::cmd::status::Snapshot {
            overlaps: [(
                "CLAUDE.md".to_string(),
                vec!["ledger[uncommitted now]".to_string(), "alerts[committed]".to_string()],
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        })
        .contains("overlap: CLAUDE.md held by ledger[uncommitted now], alerts[committed]");
    Probe {
        name: "status: the overlap line names only holders that can collide, so six clean ones produce no line",
        red_fires: red,
        green_passes: green,
    }
}

/// air-kcns: a refusal names the command it refused, so `bd close` is not reported as a failed
/// hand-over.
///
/// The gate said `handover refused for w3 at <sha>` whatever it had matched. An adopter's
/// worker met that on `bd close ad-c17zy` having just run its hand-over successfully, and the
/// reading it invites — the hand-over failed, run it again — costs 350 to 700 seconds there and
/// fixes nothing. The sentence was a **true statement about the gate and a false one about what
/// the reader had just done**: it named the thing that succeeded and reported it as failing.
///
/// Both formatters are pinned, on air-jy99's evidence that one line had six renderings. They do
/// NOT get the same treatment, and that is the finding rather than an omission: `stop_message`
/// runs at Stop, where **no command was refused**, so naming one there would invent a subject.
/// Its "handover would refuse" is correct and is asserted to stay.
///
/// Red: a real `handover_gate` refusal triggered by `bd close` names `bd close` and does not
/// call itself a hand-over. Green: the advisory form says "would be refused" rather than
/// "would refuse"; `air handover`, which has no command, keeps the old subject; the Stop
/// message names no command; and what the gate MATCHES is unchanged for all three forms.
fn probe_refusal_names_the_command_it_refused() -> Probe {
    use crate::cmd::hook::{handover_command_label, is_handover_command};
    use air_hooks::{HookOutcome, handover_verdict, stop_message};

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
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("zz-1", "w3", &[], "t0")
            .map_err(|e| e.to_string())?;

        // The reported case, through the real gate: no green at HEAD, refused on a close.
        let refused = handover_gate(&l, "w3", &dir, "bd close zz-1 --reason \"x\"", true)?;
        let reason = match &refused.outcome {
            HookOutcome::Block { reason } => reason.clone(),
            _ => String::new(),
        };
        // Both namings are the red half: they are the assertions the mutation must take, and
        // putting the advisory one in green would let one mutation claim two halves.
        let red_block = reason.contains("bd close refused for w3 at")
            // The false statement is gone, not merely joined by a true one.
            && !reason.contains("handover refused");

        // Advisory: same subject, and a mode that reads as a prediction about the command.
        let advisory = handover_gate(&l, "w3", &dir, "bd close zz-1", false)?;
        let ctx = match &advisory.outcome {
            HookOutcome::Allow { context } => context.clone().unwrap_or_default(),
            HookOutcome::Block { reason } => reason.clone(),
        };

        // No command to name: `air handover` IS the hand-over query, so the subject stands.
        let f = crate::cmd::handover::facts(&l, "w3", &dir, Some("zz-1"), true)
            .map_err(|e| e.to_string())?;
        let v = handover_verdict(&f);

        let red = red_block && ctx.contains("bd close would be refused for w3 at");

        // Every clause below holds with the subject hardcoded again, so the mutation cannot
        // take both halves: these are the cases that must NOT gain a command.
        let green = v.message.starts_with("handover would refuse for w3 at")
            // Stop has no command, so it must not invent one.
            && !stop_message(&v, "w3", &f.head).contains("bd close")
            && stop_message(&v, "w3", &f.head).contains("handover would refuse")
            // What the gate MATCHES is unchanged, and each form labels itself.
            && handover_command_label("bd close zz-1") == Some("bd close")
            && handover_command_label("bd update zz-1 -s closed") == Some("bd update -s closed")
            && handover_command_label("bd update zz-1 --status=awaiting_review")
                == Some("bd update -s awaiting_review")
            && handover_command_label("git commit -m x").is_none()
            && is_handover_command("bd close zz-1")
            && !is_handover_command("git merge main");
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "gate: a refusal names the command it refused, and the Stop message, which refused nothing, does not",
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
                bead: Some((*b).to_string()),
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

/// air-29a, then 2026-09-14: who may land or close comes from the launcher's `AIR_ROLE`.
///
/// The incident: a worker ran `air --repo <main> land --all` from its worktree on 2026-08-22 and
/// landed, because the check read `--repo`. The fix read the directory the process ran in, which
/// the owner ruled out on 2026-09-14: nothing decides permissions by directory, since any role
/// may work in a worktree. The role is now the launcher's word, which neither `--repo` nor the
/// directory nor the command's spelling can change.
///
/// Red: a worker is refused both commands, and an unknown `AIR_ROLE` counts as a worker. Green:
/// the coordinator and the owner (no `AIR_ROLE`, a shell Air did not start) may run both.
fn probe_land_role_is_the_launchers() -> Probe {
    use crate::cmd::close::may_close;
    use crate::cmd::land::may_land;
    use crate::cmd::role_from;

    let red = may_land("worker").is_err()
        && may_close("worker").is_err()
        && may_land(role_from(Some("lane-typo"))).is_err();
    let green = may_land(role_from(Some("coordinator"))).is_ok()
        && may_land(role_from(None)).is_ok()
        && may_close(role_from(None)).is_ok();
    Probe {
        name: "land and close: the role is the launcher's AIR_ROLE, so no directory and no --repo makes a worker the coordinator",
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
        && line.contains("ready: 11 (0 claimable; 2 epic(s), not claimable; 9 owner-labelled")
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
        // air-155w: this pinned `air record verify -- make verify` and `LAST` inside a check
        // that is correctly flow_dependent: false — writing a digest IS flow-free. The
        // forbidden repair rode in on the ORDER NOTE rather than on the check, which is why
        // the flag could not reach it (alerts, enumerating the ten sites). What the note has
        // to say is the ORDER and the reason, with who takes the green left to the flow.
        red_fires: w.contains("moves HEAD off")
            && w.contains("BEFORE the green is taken")
            && w.contains("must do it last")
            && !w.contains("air record verify -- make verify"),
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
        // air-155w: this asserted the refusal names `air record verify -- make verify`,
        // which is the clause a verify lane forbids. What must be true is that the refusal
        // names the CONDITION and the check, so a worker knows what is missing without being
        // told to run something their flow may not allow.
        let red_fires = matches!(&red.outcome, HookOutcome::Block { reason }
            if reason.contains("verify-green-at-head")
                && reason.contains("a green at this head")
                && !reason.contains("air record verify -- make verify"));
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
            main_sha: None,
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
                has_transcript: true,
                stopped: None,
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
                has_transcript: true,
                stopped: None,
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
                has_transcript: true,
                stopped: None,
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
        .is_some_and(|i| record_message(&ledger, "alpha", "worker", i).is_ok());
    let after_one = ledger.messages().unwrap_or_default();
    let red_fires = session_row
        && first
        && matches!(after_one.as_slice(), [m]
            if m.content == "the plan is X" && m.to == "main" && m.bytes == 13
            && m.from_worker == "alpha" && m.from_role == "worker" && m.project == "air"
            && m.session_id == "s-msg");
    let second = input
        .as_ref()
        .is_some_and(|i| record_message(&ledger, "alpha", "worker", i).is_ok());
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
/// is two lookups: not already landable (green at a head containing main), and a `Bead:`
/// trailer names a bead the worker holds. Being behind main is NOT a reason to be absent
/// (an adopter's fleet protocol, 2026-09-25): the lane merges main forward at the cut.
///
/// Red: a branch that committed a claimed bead is listed, on the status line with its beads,
/// and still listed once main has moved past it, green or not. Green: the same branch landable
/// on its own is absent; a branch naming only an unclaimed bead is absent; and every absence
/// carries the first fact it lacks.
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
        precheck_required: false,
        precheck_green_at_head: false,
    };
    let ready = batch_ready_rule(&base);
    let line = render_for_probe(&Snapshot {
        batch_ready: ready.clone().into_iter().collect(),
        ..Default::default()
    });
    let red_fires = ready
        .as_ref()
        .is_ok_and(|b| b.worker == "alpha" && b.beads == ["zz-1"])
        && line.contains("batch-ready: alpha at abcdef12 (zz-1)\n")
        && batch_ready_rule(&BatchFacts {
            contains_main: false,
            ..base.clone()
        })
        .is_ok()
        && batch_ready_rule(&BatchFacts {
            contains_main: false,
            green_at_head: true,
            ..base.clone()
        })
        .is_ok();

    let green = batch_ready_rule(&BatchFacts {
        green_at_head: true,
        ..base.clone()
    });
    let unclaimed = batch_ready_rule(&BatchFacts {
        held: vec![],
        ..base.clone()
    });
    let green_passes = green.as_ref().is_err_and(|n| n.check == "green-at-head")
        && unclaimed
            .as_ref()
            .is_err_and(|n| n.check == "no-claimed-bead" && n.detail.contains("zz-1 zz-9"))
        && !render_for_probe(&Snapshot::default()).contains("batch-ready");
    Probe {
        name: "status: batch-ready is two facts (not landable on its own, a claimed bead named); a branch behind main stays listed, a landable one is absent with its reason",
        red_fires,
        green_passes,
    }
}

/// Precheck (2026-09-25): where `.claude/air.json` declares `"precheck": true`, a branch is batch-ready
/// only with a green `precheck` run at its head. An adopter gated its lane on a precheck log
/// file and "checked at <sha>" messages, 2026-09-05..07, and cut a worker before its check
/// finished; a ledger row at the head is the fact that log stood for.
///
/// Red: declared and no green precheck at the head, the branch is absent with `no-precheck`
/// and a fixing line naming `air record precheck --`. Green: declared with a green precheck, it
/// is listed; undeclared, the rule is exactly what it was, precheck or none.
///
/// The mutation that made it red, seen: the arm reading `f.precheck_green_at_head` instead of
/// `!f.precheck_green_at_head`, which lists the unchecked head and refuses the checked one.
fn probe_batch_ready_wants_a_precheck_where_declared() -> Probe {
    use crate::cmd::status::{BatchFacts, batch_ready_rule};

    let base = BatchFacts {
        worker: "alpha".into(),
        head: "abcdef1234567890".into(),
        contains_main: true,
        green_at_head: false,
        carried: vec!["zz-1".into()],
        held: vec!["zz-1".into()],
        precheck_required: true,
        precheck_green_at_head: false,
    };
    let red_fires = batch_ready_rule(&base).is_err_and(|n| {
        n.check == "no-precheck"
            && n.detail.contains("alpha at abcdef12")
            && n.detail.contains("air record precheck --")
    });
    let checked = batch_ready_rule(&BatchFacts {
        precheck_green_at_head: true,
        ..base.clone()
    });
    let undeclared = batch_ready_rule(&BatchFacts {
        precheck_required: false,
        ..base.clone()
    });
    let green_passes =
        checked.is_ok_and(|b| b.beads == ["zz-1"]) && undeclared.is_ok_and(|b| b.beads == ["zz-1"]);
    Probe {
        name: "status: where the repo declares a precheck, batch-ready wants a green one at the head (`no-precheck` names the command); undeclared, the rule is unchanged",
        red_fires,
        green_passes,
    }
}

/// Precheck (2026-09-25), the other half: a `precheck` is a worker's cheap check, and the close gate,
/// the landable list and `air land` all ask for a `verify` green. They ask through
/// `green::at` and the ledger's kind-keyed queries, so this is one question: does a green
/// precheck at a sha answer "is this sha verify-green"? It must not; the reverse holds too.
///
/// Red: a green precheck recorded at a sha is found as a precheck green there. Green: the same
/// sha has no verify green, by commit or by tree, and no verify candidate for a batch.
///
/// The mutation that made it red, seen: `Kind::Precheck => "precheck"` in `Kind::as_str`
/// written as `Kind::Precheck => "verify"`, so the row is stored as a verify.
fn probe_a_precheck_green_is_never_a_verify_green() -> Probe {
    let (red_fires, green_passes) = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_verify(&VerifyRun {
            id: new_id(),
            worker: "alpha".into(),
            sha: "c0ffee00".into(),
            kind: Kind::Precheck,
            exit_code: 0,
            trigger: "record".into(),
            failing_step: None,
            started_at: "2026-09-25T00:00:00Z".into(),
            finished_at: "2026-09-25T00:00:01Z".into(),
            log_path: None,
            command: Some("make precheck".into()),
            duration_ms: Some(5_000),
            output_bytes: Some(10),
            dirty: false,
            tree: Some("7ree".into()),
            members: vec![],
            main_sha: None,
        })
        .map_err(|e| e.to_string())?;
        let red = l
            .green_at("c0ffee00", Some("7ree"), Kind::Precheck)
            .map_err(|e| e.to_string())?
            .is_some();
        let green = l
            .green_at("c0ffee00", Some("7ree"), Kind::Verify)
            .map_err(|e| e.to_string())?
            .is_none()
            && l.latest_greens(Kind::Verify, 10)
                .map_err(|e| e.to_string())?
                .is_empty()
            && l.runs_at("c0ffee00", Kind::Verify)
                .map_err(|e| e.to_string())?
                == (0, 0);
        Ok((red, green))
    })()
    .unwrap_or_else(blocked);
    Probe {
        name: "ledger: a green precheck is found as a precheck and never as a verify green, by commit, by tree, or as a batch candidate",
        red_fires,
        green_passes,
    }
}

/// The cut as a program (2026-09-25): which member a pairwise conflict drops is decided by the order rule, never
/// by the order the shas were typed. An adopter's lane skipped its dry-merge once on 2026-09-07
/// and typing order decided which member "conflicted" (`batch_cut.rs` module doc).
///
/// Red: members handed over NEWEST first, beta and gamma conflicting with each other and delta
/// with main. The older-ready gamma is kept and beta dropped naming gamma and the path; delta is
/// dropped naming main. Green: the clean member is kept, a pair drops exactly one of its two
/// members, and the same set in the other input order gives the same answer.
///
/// The mutation that made it red, seen: `pre_check` without its `order` call, which keeps beta
/// (typed first) and drops gamma.
fn probe_batch_cut_drops_by_the_order_rule() -> Probe {
    use crate::cmd::batch_cut::{Candidate, pre_check};
    let c = |w: &str, at: i64| Candidate {
        worker: w.into(),
        head: format!("{w}-sha"),
        beads: vec![format!("zz-{w}")],
        ready_at: at,
    };
    // Newest first, so typing order and the rule disagree.
    let typed = vec![
        c("delta", 40),
        c("beta", 30),
        c("alpha", 20),
        c("gamma", 10),
    ];
    let conflict = |ours: &str, theirs: &str| -> Result<Vec<String>, String> {
        let pair = [ours, theirs];
        Ok(
            if pair.contains(&"beta-sha") && pair.contains(&"gamma-sha") {
                vec!["shared.txt".into()]
            } else if pair == ["main-sha", "delta-sha"] {
                vec!["m.txt".into()]
            } else {
                vec![]
            },
        )
    };
    let Ok((kept, dropped)) = pre_check(typed.clone(), "main-sha", conflict) else {
        return Probe {
            name: "batch cut: a pairwise conflict drops the later-ready member, naming the other side and the paths, whatever order the members arrive in",
            red_fires: false,
            green_passes: false,
        };
    };
    let names = |v: &[Candidate]| v.iter().map(|m| m.worker.clone()).collect::<Vec<_>>();
    let red_fires = dropped.iter().any(|d| {
        d.worker == "beta"
            && d.against == "gamma"
            && d.against_sha == "gamma-sha"
            && d.paths == ["shared.txt"]
    }) && dropped
        .iter()
        .any(|d| d.worker == "delta" && d.against == "main" && d.paths == ["m.txt"]);
    let mut reversed = typed;
    reversed.reverse();
    let again = pre_check(reversed, "main-sha", conflict);
    let green_passes = names(&kept) == ["gamma", "alpha"]
        && dropped.len() == 2
        && again.is_ok_and(|(k, d)| k == kept && d == dropped);
    Probe {
        name: "batch cut: a pairwise conflict drops the later-ready member, naming the other side and the paths, whatever order the members arrive in",
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
        refused_command: None,
        head: "0123456789abcdef".into(),
        green_at_head: true,
        tree_green: None,
        batch_green: None,
        batch_predates: None,
        batch_absent: None,
        batch_absent_fix: None,
        last_green_sha: None,
        main_is_ancestor: true,
        main_sha: "fedcba9876543210".into(),
        main_moved: None,
        bead_claimed_or_carried: true,
        runs_at_head: (1, 0),
        digest_present: None,
        digest_untracked: false,
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
    // air-155w: this pinned `git merge main && air record verify -- make verify` as
    // "unchanged", which is what kept the forbidden clause alive through air-avj. The
    // DIAGNOSIS is the part that must not change — the adopter counts refusals by its phrase —
    // and the fix is the part that had to. So the assertion inverts: the merge is still named,
    // because it is required under both flows, and no fix on a flow-dependent check asserts
    // recording a green, because a lane forbids exactly that.
    let fix_is_flow_safe = v
        .missing
        .iter()
        .any(|m| m.check == "main-merged" && m.fix.contains("git merge main") && m.flow_dependent)
        && v.missing
            .iter()
            .filter(|m| m.flow_dependent)
            .all(|m| !m.fix.contains("air record verify --"));
    let mut g = base_facts();
    g.main_is_ancestor = false;
    let plain = handover_verdict(&g);
    Probe {
        name: "gate: a refusal after a landing names the landing that moved main, when and from whom, and its fix asserts no repair a verify lane forbids",
        red_fires: red,
        green_passes: fix_is_flow_safe
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
            // air-dwq5: a leading `-` is a FLAG, not a subcommand, and belongs to a namespace
            // this scanner does not enumerate. It first mattered when `air --version` became
            // advice worth shipping; before that no shipped string named a flag in command
            // position, so the scanner never had to tell the two apart. Validating flags too
            // would be a bigger check than the failure asks for (air-w91 was a subcommand that
            // did not exist), and it is not built until one is shipped that does not exist.
            if !word.is_empty() && !word.starts_with('-') {
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
            main_sha: None,
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
        .map(|(code, _, _)| code)
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
            main_sha: None,
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
            main_sha: None,
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
        // air-155w: the refusal names the CONDITION, not a command a verify lane forbids.
        // What this probe is about is that the env reached the hook and the gate refused, so
        // it asserts the refusal happened and names its check.
        let refused = code == 2 && err.contains("verify-green-at-head");
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
/// Green: a real `air status` over three such claims runs exactly four bd processes — the
/// in-progress list, ONE `show` naming all three, `ready`, and the unfinished list the
/// ancestor-deadlock scan reads (air-btz) — and NO `dep list`, because no bead here has a
/// parent and the scan's second call is gated on one that also has an edge. Both halves are
/// asserted: the count is what a lane notices when a bd call is added, and the absent
/// `dep list` is the gate that keeps the common repo at one call rather than two.
/// Every claim also ends where the per-bead loop put it: the closed bead released as
/// `closed`, the reopened one as `reconciled`, the awaiting_review one kept and marked
/// handed over. Outputs, not only the count.
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
        // air-btz added the fourth: the unfinished list the deadlock scan reads. The fifth,
        // `dep list`, must NOT be here — nothing in this repo has a parent, so the gate holds
        // and the scan costs one process rather than two.
        let four_processes = log.len() == 4
            && log.iter().filter(|l| l.starts_with("list ")).count() == 2
            && !log.iter().any(|l| l.starts_with("dep "));
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
        Ok((red, one_show && four_processes && outputs))
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
        main_sha: None,
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
            // Pinned identity (air-dws): launched as a worker. The role is the launcher's,
            // never the worktree's (owner, 2026-09-14).
            let mut child = air_command(&exe, &wt)
                .arg("hook")
                .env("AIR_ROLE", "worker")
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

/// A stub bd that costs 100 ms per id, slept ONCE for the set (a `sleep` per id costs 200 ms of
/// spawn each on a loaded machine, which is noise, not the shape): fourteen ids cost 1.4 s.
/// `show` answers every id with one acceptance clause; `close` answers nothing. Returns the
/// directory (remove it) and the script.
fn per_id_stub_bd() -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let script = dir.join("bd");
    std::fs::write(
        &script,
        r###"#!/bin/sh
cmd="$1"; shift; out=''; n=0
for id in "$@"; do case "$id" in --*) break;; esac; n=$((n+1))
  out="$out${out:+,}{\"id\":\"$id\",\"status\":\"open\",\"labels\":[],\"description\":\"## Acceptance Criteria\\n- it lands\"}"
done
perl -e "select(undef,undef,undef,$n*0.1)"
case "$cmd" in
  show) printf '%s\n' "[$out]";;
esac
exit 0
"###,
    )
    .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok((dir, script))
}

/// air-fzv, then air-8lj8: every multi-id bd call's budget scales with the id count, from ONE
/// function (`air_bd::budget_for`) inside the client. It was a flat 10 s for the whole id set;
/// the adopter's fourteen-bead batch under the verify lane was refused until they raised
/// `AIR_BD_TIMEOUT_MS` by hand, and `air close <ids…>` never got the fix the acceptance read
/// got. Measured here 2026-09-06: one `bd show` with fourteen ids takes 21 s, one id 2 s.
///
/// The probe scales the seconds down to milliseconds and keeps the shape: a stub bd that costs
/// a fixed time per id, fourteen ids, and two clients with the same base. Red: with no per-id
/// allowance, both the acceptance read (`show_all`) and `air close` (`close_all`) are refused,
/// and each refusal names the id count, the budget and `AIR_BD_TIMEOUT_MS`. Green: with the
/// allowance, the same calls answer, and the shipped default for fourteen ids is above what
/// bd measured.
fn probe_acceptance_budget_scales_with_ids() -> Probe {
    use crate::cmd::status::acceptance_with;
    use std::time::Duration;

    let res = (|| -> Result<(bool, bool), String> {
        let (dir, script) = per_id_stub_bd()?;
        let ids: Vec<String> = (1..=14).map(|i| format!("zz-{i:02}")).collect();
        let client = |per_id: Duration| air_bd::BdCli {
            bin: script.clone(),
            cwd: dir.clone(),
            timeout: Duration::from_millis(500),
            per_id,
            label: air_ledger::budgets::BD_ACCEPTANCE,
        };
        // The old shape: one flat budget whatever the count.
        let flat = client(Duration::ZERO);
        let refused = acceptance_with(&flat, &ids, false);
        let read_red = matches!(&refused, Err(m) if m.contains("14 id(s)")
            && m.contains("within a budget of 0.5 s")
            && m.contains("AIR_BD_TIMEOUT_MS"));
        let close_err = air_bd::WorkLedger::close_all(&flat, &ids, "landed", "")
            .err()
            .map(|e| crate::cmd::close::refusal(&e, ids.len(), false));
        let close_red = matches!(&close_err, Some(m) if m.contains("14 id(s)")
            && m.contains("within a budget of 0.5 s")
            && m.contains("AIR_BD_TIMEOUT_MS"));
        // The new shape: same base, plus an allowance per id. THIRTY times the stub's per-id
        // cost: the probe is about the SHAPE (base + per_id x n), the red half above already
        // proves a flat budget refuses, and the margin is free because a budget is a ceiling
        // and the stub answers in 1.4 s whatever it is set to. Three times was measured at
        // load 53 and lost at load 186; ten times was lost again at load 145 (air-g7e), where
        // this probe failed for the machine's reasons and took four unrelated mutations down
        // with it as "vacuous".
        let scaled = client(Duration::from_millis(3000));
        let answered = acceptance_with(&scaled, &ids, false);
        let all_read = matches!(&answered, Ok(c) if c.len() == 14
            && c.iter().all(|clauses| clauses.len() == 1));
        let closed = air_bd::WorkLedger::close_all(&scaled, &ids, "landed", "").is_ok();
        let shipped = air_bd::BdCli::new(&dir);
        let covers_measured =
            shipped.budget(14) > Duration::from_secs(21) && shipped.budget(14) > shipped.budget(1);
        let _ = std::fs::remove_dir_all(&dir);
        Ok((read_red && close_red, all_read && closed && covers_measured))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "bd: every multi-id call's budget grows with the id count (acceptance read and air close), and the refusal names the count, the budget and AIR_BD_TIMEOUT_MS",
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
        // The verification lane claims nothing, so it is offered nothing (an adopter's fleet protocol, 2026-09-25).
        && stop_nudge("lane", false, &ready, false).is_none()
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

/// An empty scratch directory for a probe. The caller runs [`probe_git`] in it and removes it.
fn probe_repo() -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// `git -C <dir> <args>`, with an identity, failing loudly. The probes below build real
/// histories rather than fixtures because both facts air-9ij fixed are ancestry facts.
fn probe_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
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
}

/// air-9ij, limb 3, the one the adopter measured: a batch green stopped covering the bead it
/// covers the moment main moved. `contains main` was asked of CURRENT main at query time, so a
/// green recorded over the main of its moment was disqualified by any later write to main — a
/// landing, or the coordinator's own prose commit, which is what invalidated an adopter's
/// whole batch on 2026-09-06. The window was not closed by the worker; it was closed by
/// somebody else. The question is now asked of the run's recorded `main_sha`.
///
/// Red (the declared mutation is on the row: `main_sha: None`, exactly a pre-v19 row, which
/// can only ask about current main): with main moved, the batch green covers nothing. Green:
/// the same green with its recorded main covers the bead after main has moved, and a green
/// naming a main it does not contain covers nothing, so the gate is not widened.
fn probe_batch_green_survives_main_moving_under_it() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = probe_repo()?;
        let g = |args: &[&str]| probe_git(&dir, args);
        let out = (|| -> Result<(bool, bool), String> {
            g(&["init", "-q", "-b", "main"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "base"])?;
            let base = g(&["rev-parse", "HEAD"])?;
            // The worker's commit for fd-1, then a lane batch over main plus that commit.
            g(&["checkout", "-q", "-b", "w"])?;
            g(&[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "work\n\nBead: fd-1\n",
            ])?;
            g(&["checkout", "-q", "-b", "lane", "main"])?;
            g(&["merge", "-q", "--no-ff", "w", "-m", "batch: w"])?;
            let batch = g(&["rev-parse", "HEAD"])?;
            // Main moves after the cut by an ordinary commit: nothing landed, nothing merged.
            g(&["checkout", "-q", "main"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "docs: prose"])?;
            let moved = g(&["rev-parse", "HEAD"])?;
            g(&["checkout", "-q", "w"])?;
            g(&["merge", "-q", "main", "-m", "merge main"])?;

            let covered = |main_sha: Option<&str>| -> Result<bool, String> {
                let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
                l.record_verify(&VerifyRun {
                    id: new_id(),
                    worker: "lane".into(),
                    sha: batch.clone(),
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
                    main_sha: main_sha.map(str::to_string),
                })
                .map_err(|e| e.to_string())?;
                Ok(crate::cmd::batch::for_bead(&l, &dir, "fd-1")?
                    .covering
                    .is_some())
            };
            let red = !covered(None)?;
            let green = covered(Some(&base))? && !covered(Some(&moved))?;
            Ok((red, green))
        })();
        std::fs::remove_dir_all(&dir).ok();
        out
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "gate: a batch green goes on covering its bead after main moves, because `contains main` is asked of the main the run was recorded over",
        red_fires: red,
        green_passes: green,
    }
}

/// air-9ij, limb 1, and the judgement it forced: **when every commit of a bead is already in
/// main, the close passes.** `main..HEAD` is empty then, and reading that as "this bead has no
/// commits" refused the close of a bead Air had itself landed, in any repo that keys green by
/// commit. The landing gate already demanded a green at a head containing main, and the commit
/// main is fast-forwarded onto carries that green's tree (air-odv), so the proof this gate
/// asks for is the proof the work arrived with. Requiring a fresh green naming the merge would
/// have Air refuse the close it nags for as `landed-not-closed`.
///
/// Red (declared mutation: the landing row says `refused` rather than `landed`): the work is
/// not on main, so an empty range closes nothing. Green: the `landed` row closes it, a row
/// that named a different bead does not, and a landing whose merge commit main no longer
/// contains does not either — a worker that claimed a bead and committed nothing has no row
/// and still has nothing to close on.
fn probe_a_landed_bead_closes_on_its_landing() -> Probe {
    use air_ledger::landings::Landing;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = probe_repo()?;
        let g = |args: &[&str]| probe_git(&dir, args);
        let out = (|| -> Result<(bool, bool), String> {
            g(&["init", "-q", "-b", "main"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "base"])?;
            // A commit main never gets, to stand for a landing that was rewound.
            g(&["checkout", "-q", "-b", "stray"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "stray"])?;
            let stray = g(&["rev-parse", "HEAD"])?;
            g(&["checkout", "-q", "-b", "w", "main"])?;
            g(&[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "work\n\nBead: fd-1\n",
            ])?;
            g(&["checkout", "-q", "main"])?;
            g(&["merge", "-q", "--no-ff", "w", "-m", "land: w"])?;
            let merge = g(&["rev-parse", "HEAD"])?;
            // The worker's next `git merge main` fast-forwards: main..HEAD is now empty.
            g(&["checkout", "-q", "w"])?;
            g(&["merge", "-q", "--ff-only", "main"])?;
            let empty = crate::cmd::batch::bead_commits(&dir, "fd-1").is_empty();

            let landed = |result: &str, beads: &[&str], at: &str| -> Result<bool, String> {
                let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
                l.record_landing(&Landing {
                    id: new_id(),
                    worker: "lane".into(),
                    sha: "wwww".into(),
                    tip_sha: None,
                    result: result.into(),
                    failing_step: None,
                    verify_run_id: None,
                    attempt_no: 1,
                    beads: beads.iter().map(|b| (*b).to_string()).collect(),
                    open_beads: vec![],
                    merge_commit: Some(at.to_string()),
                    pid: None,
                    started_at: "t0".into(),
                    finished_at: "t1".into(),
                    despite_inflight: vec![],
                    members: vec![],
                })
                .map_err(|e| e.to_string())?;
                Ok(crate::cmd::batch::for_bead(&l, &dir, "fd-1")?
                    .landed
                    .is_some())
            };
            let red = empty && !landed("refused", &["fd-1"], &merge)?;
            let green = landed("landed", &["fd-1"], &merge)?
                && landed("landed-refuted", &["fd-1"], &merge)?
                && !landed("landed", &["fd-2"], &merge)?
                && !landed("landed", &["fd-1"], &stray)?;
            Ok((red, green))
        })();
        std::fs::remove_dir_all(&dir).ok();
        out
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "gate: a bead whose every commit is already in main closes on the landing that put it there, and only on one that named it and that main still contains",
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
        // (exit code, stderr) of one PreToolUse Edit from `cwd` at `path`, as `role`.
        let edit = |cwd: &Path, path: &Path, role: &str| -> Result<(i32, String), String> {
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
                .env("AIR_ROLE", role)
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
        let (out_code, out_err) = edit(&wt, &outside, "worker")?;
        let (in_code, _) = edit(&wt, &wt.join("src").join("a.rs"), "worker")?;
        // The gap the harness's isolation did close: a hand-written climb out of the worktree.
        let (climb_code, _) = edit(
            &wt,
            &wt.join("..").join("..").join("..").join("src").join("a.rs"),
            "worker",
        )?;
        // The coordinator editing the same file the worker was refused, from inside the worker's
        // worktree: the role decides, not the directory.
        let (main_code, _) = edit(&wt, &outside, "coordinator")?;
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
        name: "hook: a worker's edit outside its worktree is denied naming the path; inside is allowed and the coordinator is never fenced, wherever it runs",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-g5o: Metis reaches the coordinator's session and no worker's.
///
/// The owner asked whether making the planning rule programmatic is "what metis does
/// basically". It is not: Metis enforces forward-only phases on its own documents and does not
/// enforce that anyone plans in it (`docs/design.md` §11, the Metis row). The harness has
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

/// air-ej4: `air init --write` writes a `Makefile` into a repo that has none, because Air's one
/// refusal reads a recorded green and a repo with no verify command has nothing to record. The
/// hazard the scaffold creates is the opposite one: a target that Air wrote and nobody edited
/// would let `air record verify -- make verify` record a green for an empty check, and the close
/// gate would pass it. So the scaffolded target must FAIL until a human replaces its body.
///
/// Red: the target Air writes exits non-zero, and says which file to edit. Green: the same
/// Makefile with the placeholder recipe swapped for a real command passes, so what fails is the
/// placeholder and not a Makefile Air wrote wrong.
///
/// `make` when it is on PATH (the real thing); the recipe under `sh` otherwise, which is what
/// make does with a one-line recipe. A machine without make must not produce a false red.
fn probe_scaffolded_verify_fails_until_edited() -> Probe {
    use crate::cmd::init::{makefile_stub, scaffold};

    fn verify_succeeds(makefile: &str) -> Result<bool, String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("Makefile"), makefile).map_err(|e| e.to_string())?;
        let ran = Command::new("make")
            .args(["-C", &dir.to_string_lossy(), "verify"])
            .output();
        let ok = match ran {
            Ok(o) => o.status.success(),
            // No make here: run the recipe body itself, minus make's tab and `@` prefix.
            Err(_) => {
                let body: String = makefile
                    .lines()
                    .filter(|l| l.starts_with('\t'))
                    .map(|l| l.trim_start_matches('\t').trim_start_matches('@'))
                    .collect::<Vec<_>>()
                    .join("\n");
                Command::new("sh")
                    .args(["-c", &body])
                    .output()
                    .map_err(|e| e.to_string())?
                    .status
                    .success()
            }
        };
        let _ = std::fs::remove_dir_all(&dir);
        Ok(ok)
    }

    let written = makefile_stub();
    // "Edited": the human has put a real check where the placeholder was. Anchored on the tab,
    // so this cannot silently stop replacing anything if the wording of the echo changes.
    let edited: String = written
        .lines()
        .map(|l| if l.starts_with('\t') { "\t@true" } else { l })
        .collect::<Vec<_>>()
        .join("\n");
    let res = (|| -> Result<(bool, bool), String> {
        Ok((verify_succeeds(&written)?, verify_succeeds(&edited)?))
    })();
    let (placeholder_passed, edited_passed) = res.unwrap_or((true, false));
    Probe {
        name: "init: the verify target `air init` scaffolds FAILS until it is edited, so a fresh repo cannot record a green for an empty check",
        red_fires: !placeholder_passed
            && written.contains("Makefile:verify")
            && scaffold(None, false, ("docs/journal", false))
                .iter()
                .any(|i| i.path == "Makefile" && i.create),
        // Present means untouched, both ways round: a Makefile with a verify target and one
        // without are both left alone, and only the printed sentence differs.
        green_passes: edited_passed
            && scaffold(Some("verify:\n\t@true\n"), true, ("docs/journal", true))
                .iter()
                .all(|i| !i.create)
            && scaffold(Some("build:\n\t@true\n"), false, ("docs/journal", true))
                .iter()
                .any(|i| i.path == "Makefile" && !i.create && i.note.contains("NO `verify`")),
    }
}

/// air-84u (owner, 2026-09-06: "the coordinator isn't automatically doing that thing to turn
/// epics into beads"). `air status` already counted the ready epics; the count is a number the
/// coordinator then has to resolve against bd by hand, and twice on 2026-09-06 nobody did.
/// air-80x sat undecomposed for hours and is ready again with all six children closed. The
/// line names which epic, so the count becomes an action.
///
/// Red (declared mutation: `all(closed)` becomes `any(closed)`): an epic with one open child
/// is named, which is the one thing this must never do — a line that names an epic somebody is
/// working on costs its own credibility. Green: an epic whose children are all closed is
/// named with the count, an epic with no children at all is named with 0, a child in any
/// status but `closed` keeps its epic silent, and a snapshot with no such epic prints no line.
fn probe_epic_with_no_open_children_is_named() -> Probe {
    use crate::cmd::status::{EpicToDecompose, Snapshot, render_for_probe, to_decompose};

    let kid = |status: &str| air_bd::Issue {
        id: "zz-1".into(),
        status: status.into(),
        ..air_bd::Issue::default()
    };
    let closed = [kid("closed"), kid("closed")];

    // RED: an epic with work under it stays silent, in every status that is not `closed`.
    let red = ["open", "in_progress", "blocked", "awaiting_review"]
        .iter()
        .all(|s| to_decompose("zz-e", &[kid("closed"), kid(s)]).is_none());

    let named = to_decompose("zz-e", &closed);
    let never = to_decompose("zz-e", &[]);
    let line = render_for_probe(&Snapshot {
        epics_to_decompose: Some(vec![EpicToDecompose {
            epic: "zz-e".into(),
            closed_children: 6,
        }]),
        ..Snapshot::default()
    });
    let silent = render_for_probe(&Snapshot::default());
    let green = named
        == Some(EpicToDecompose {
            epic: "zz-e".into(),
            closed_children: 2,
        })
        // An epic nobody has ever decomposed is the same duty, and says 0.
        && never
            == Some(EpicToDecompose {
                epic: "zz-e".into(),
                closed_children: 0,
            })
        && line.contains("epic ready to decompose: zz-e (0 open children, 6 closed)")
        && !silent.contains("epic ready to decompose");

    Probe {
        name: "status: a ready epic with no open child is named with its closed count; one with work under it, in any status but closed, is not",
        red_fires: red,
        green_passes: green,
    }
}

/// air-5nh: `air audit` prints re-claim churn, so "a worker took a bead it could not start"
/// is a number instead of an argument. air-69u measured it once by hand (4 of 153 claims,
/// 2.6%) and used it to refuse two mechanisms: a filing command wrapping `bd create --after`,
/// and a refusal on a child with no ordering edge. A number that decides that has to be
/// re-runnable, and it has to read the right population.
///
/// Red: the shape air-69u found comes back out. Two workers claim one bead and release it
/// `owner-gated` inside half a minute each; a third claim is released fast for an unrelated
/// reason; a fourth is owner-gated but hours later; a fifth is still held. The counts separate
/// all five, and the printed section carries the 10% threshold beside the rate.
///
/// Green: the RATE reads the owner-gated-inside-a-minute population and nothing wider, so a
/// fast release for an ordinary reason cannot inflate the number that would justify the
/// mechanism. And a window with no claims reports no rate at all rather than 0%, because
/// unmeasured and zero are different facts.
fn probe_reclaim_churn_reads_the_owner_gated_population() -> Probe {
    use crate::cmd::audit::{churn_of, render_churn};

    let row = |bead: &str, worker: &str, from: &str, to: Option<&str>, why: Option<&str>| {
        (
            bead.to_string(),
            worker.to_string(),
            from.to_string(),
            to.map(str::to_string),
            why.map(str::to_string),
        )
    };
    let rows = vec![
        row(
            "air-zzj",
            "ledger",
            "2026-09-05T10:00:00Z",
            Some("2026-09-05T10:00:20Z"),
            Some("owner-gated"),
        ),
        row(
            "air-zzj",
            "verify",
            "2026-09-05T11:00:00Z",
            Some("2026-09-05T11:00:22Z"),
            Some("owner-gated: waiting on air-uko"),
        ),
        // Fast, but handed back for an ordinary reason: churn by duration, not the population
        // the threshold reads.
        row(
            "air-aaa",
            "alerts",
            "2026-09-05T12:00:00Z",
            Some("2026-09-05T12:00:30Z"),
            Some("took the wrong bead"),
        ),
        // Owner-gated, but discovered hours in: not a bead the worker could not START.
        row(
            "air-bbb",
            "launch",
            "2026-09-05T13:00:00Z",
            Some("2026-09-05T16:00:00Z"),
            Some("owner-gated"),
        ),
        // Still held. Not churn, and not counted against it either.
        row("air-ccc", "ledger", "2026-09-05T14:00:00Z", None, None),
    ];
    let c = churn_of(&rows);
    let out = render_churn(&c);
    let empty = churn_of(&[]);

    Probe {
        name: "audit: re-claim churn is counted from claims alone, and the rate reads the owner-gated-inside-a-minute population with its 10% threshold beside it",
        red_fires: (c.claims, c.released, c.within_60s, c.within_300s) == (5, 4, 3, 3)
            && c.owner_gated == 3
            && c.short.len() == 3
            // Longest first, so the tail of the distribution is readable.
            && c.short.first().is_some_and(|x| x.seconds == 30)
            && out.contains("THRESHOLD 10%")
            && out.contains("air-zzj"),
        green_passes: c.owner_gated_within_60s == 2
            && c.owner_gated_within_300s == 2
            // 2 of 5, not 3 of 5: the ordinary fast release is in `within_60s` and out of the
            // rate. A number that justifies a mechanism must not be inflated by Air.
            && c.rate.is_some_and(|r| (r - 0.4).abs() < 1e-9)
            // Unmeasured is not zero.
            && empty.rate.is_none()
            && render_churn(&empty).contains("Not zero: unmeasured"),
    }
}

/// air-btz (owner, 2026-09-06: "I just don't want that deadlock again"). A bead blocked by one
/// of its own ancestors waits forever: the ancestor cannot finish until its descendants do,
/// which is bd's hierarchy rather than an edge. An adopter lost a night to it — every P1 in
/// their queue unreachable, 42 beads offered and not one of them a P1 — because the tracker
/// renders it as "not ready yet", exactly like ordinary queueing.
///
/// **bd does not prevent this**, measured 2026-09-06 against 1.2.2, the pinned version
/// (`.claude/skills/beads/references/bd-facts.md`, "bd's dependency guard is two rules, not an ancestor walk"). Its guard is two rules and
/// neither is an ancestor walk: an existing `parent-child` row on the same pair, which always
/// catches the DIRECT parent, and a dotted-id prefix test, which catches deeper ancestors only
/// when the id encodes the chain. `bd create --graph` assigns flat ids and links by
/// `parent_key`, so a wave filed from a plan file slips both without printing anything.
///
/// Red (declared mutation: the walk stops at the parent, `1..=1`): the grandparent case is
/// missed, which is the ONLY shape that is actually reachable — bd itself already refuses
/// depth 1, so a check that only sees depth 1 sees nothing that can happen. Green: a
/// `parent-child` edge is never named (it is the hierarchy, so naming it would report every
/// child in the repo), a sibling `blocks` edge is not named, an empty repo is silent, the line
/// names both beads and a `bd dep remove` that fixes it, and the two bd calls keep their
/// shapes — one comma-separated `--status`, because a repeated `-s` silently overwrites in bd
/// 1.2.2, and one process for every id.
fn probe_ancestor_deadlock_is_named() -> Probe {
    use crate::cmd::status::{AncestorDeadlock, Snapshot, ancestor_deadlocks, render_for_probe};

    let dep = |from: &str, to: &str, ty: &str| air_bd::Dep {
        issue_id: from.into(),
        depends_on_id: to.into(),
        dep_type: ty.into(),
    };
    // E -> M -> C, the shape `bd create --graph` produces with ids that hide the chain.
    let parents: std::collections::BTreeMap<String, String> = [("C", "M"), ("M", "E"), ("S", "M")]
        .iter()
        .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
        .collect();

    // RED: C blocked by its GRANDparent E, at depth 2. bd refuses depth 1 already, so this is
    // the whole reachable subject; a check that misses it is a check that never fires.
    let found = ancestor_deadlocks(&parents, &[dep("C", "E", air_bd::BLOCKS)]);
    let red = found
        == vec![AncestorDeadlock {
            bead: "C".into(),
            ancestor: "E".into(),
            depth: 2,
        }];

    // The hierarchy edge itself is never a deadlock, and a sibling is not an ancestor.
    let hierarchy = ancestor_deadlocks(&parents, &[dep("C", "M", air_bd::PARENT_CHILD)]);
    let sibling = ancestor_deadlocks(&parents, &[dep("C", "S", air_bd::BLOCKS)]);
    let none = ancestor_deadlocks(&Default::default(), &[dep("C", "E", air_bd::BLOCKS)]);
    let line = render_for_probe(&Snapshot {
        ancestor_deadlocks: Some(vec![AncestorDeadlock {
            bead: "zz-1".into(),
            ancestor: "zz-e".into(),
            depth: 2,
        }]),
        ..Snapshot::default()
    });
    let silent = render_for_probe(&Snapshot::default());
    // One bd process per call, and the status list in ONE argument: repeating `-s` overwrites.
    let statuses = air_bd::by_statuses_argv(&["open", "in_progress"]);
    let deps = air_bd::dep_list_argv(&["a".into(), "b".into(), "c".into()]);
    let green = hierarchy.is_empty()
        && sibling.is_empty()
        && none.is_empty()
        && line.contains("deadlock: zz-1 is blocked by zz-e, its own ancestor")
        && line.contains("bd dep remove zz-1 zz-e")
        && !silent.contains("deadlock:")
        && statuses.iter().filter(|a| *a == "--status").count() == 1
        && statuses.contains(&"open,in_progress".to_string())
        && statuses.windows(2).any(|w| w == ["-n", "0"])
        && deps.starts_with(&["dep".to_string(), "list".to_string()])
        && deps.iter().filter(|a| *a == "dep").count() == 1;

    Probe {
        name: "status: a bead blocked by its own ancestor is named with the edge and the fix; the hierarchy edge and a sibling are not, and bd is asked once per call",
        red_fires: red,
        green_passes: green,
    }
}

/// air-1n3: on 2026-09-06 an account limit stopped seven interactive sessions on this machine.
/// Five had the harness's own auto-continue armed and were working again within 70 seconds of
/// the reset; two did not, and the one that also had no scheduled task ticking sat dead for 79
/// minutes. Air could see only "silent with a claim", which reads the same for a session the
/// harness is bringing back and one it has abandoned — so the coordinator took it to the owner
/// instead of acting, and the two states call for OPPOSITE actions: typing at a session with an
/// armed wait cancels the recovery.
///
/// No hook fires at a limit or at its reset. `Notification` (with the `quota_auto_resume_*`
/// types) and `StopFailure` are the nearest the harness has, and Air subscribed to neither.
///
/// Red: both events are installed, and a `quota_auto_resume_stale` notification marks the
/// session stopped with that kind, so `air status` says the harness is NOT resuming it.
///
/// Green: the two halves that separate a recorded fact from a guess. A
/// `quota_auto_resume_fired` reads as "leave it alone" rather than as one more stopped session,
/// and a `Notification` that is not about a stop at all (a permission prompt) marks NOTHING —
/// the kind is read from the harness's declared field, so a type Air has never met is logged
/// and reported as not-a-stop rather than invented into one.
fn probe_a_stopped_session_is_recorded_and_says_whether_it_recovers() -> Probe {
    use crate::cmd::hook::{is_stop_kind, stop_kind};
    use crate::cmd::install::hook_entries;
    use crate::cmd::status::stopped_phrase;
    use air_hooks::HookInput;

    let input = |json: &str| HookInput::parse(json).unwrap_or_default();
    let stale = input(
        r#"{"session_id":"s","hook_event_name":"Notification",
            "notification_type":"quota_auto_resume_stale","message":"Usage limit reset"}"#,
    );
    let fired = input(
        r#"{"session_id":"s","hook_event_name":"Notification",
            "notification_type":"quota_auto_resume_fired","message":"continuing automatically"}"#,
    );
    let perm = input(
        r#"{"session_id":"s","hook_event_name":"Notification",
            "notification_type":"permission_prompt","message":"Claude needs your permission"}"#,
    );
    let api = input(r#"{"session_id":"s","hook_event_name":"StopFailure"}"#);
    // A harness that sends no type at all, and one that sends a type Air has never seen.
    let bare = input(r#"{"session_id":"s","hook_event_name":"Notification"}"#);
    let unknown = input(
        r#"{"session_id":"s","hook_event_name":"Notification",
            "notification_type":"some_future_type"}"#,
    );

    let installed: Vec<&str> = hook_entries().into_iter().map(|(e, _)| e).collect();
    Probe {
        name: "limit: a stopped session is recorded from the harness's own notification, and status says whether the harness is bringing it back",
        red_fires: installed.contains(&"Notification")
            && installed.contains(&"StopFailure")
            && stop_kind(&stale) == "quota_auto_resume_stale"
            && is_stop_kind(&stop_kind(&stale))
            && stop_kind(&api) == "stop_failure"
            && is_stop_kind(&stop_kind(&api))
            && stopped_phrase("quota_auto_resume_stale", "T").contains("NOT resuming"),
        green_passes: is_stop_kind(&stop_kind(&fired))
            && stopped_phrase(&stop_kind(&fired), "T").contains("leave it alone")
            // Not every notification is a stop: a permission prompt, a bare one, and a type
            // from a later harness all write nothing rather than a phantom stopped session.
            && !is_stop_kind(&stop_kind(&perm))
            && !is_stop_kind(&stop_kind(&bare))
            && !is_stop_kind(&stop_kind(&unknown))
            // And the kind is the declared field, never the message text: the stale one says
            // "Usage limit reset" and the fired one says "continuing automatically", so a
            // reader of the words would have them backwards.
            && stop_kind(&fired) != stop_kind(&stale),
    }
}

/// air-lpd: what a landing records as a batch's members, on a real repo with real worktrees.
///
/// `batch::members_of` had neither a probe nor a unit test, measured by air-g7e's coverage pass
/// over every function this round added. It decides the `members` on a landing row, which is
/// the fleet's only record of what a batch contained and the thing a red batch is reported by
/// (air-80x.4). The same pass predicted air-9ij from a gap of exactly this shape before the
/// incident report arrived: the untested function was the one that broke.
///
/// Ancestry only, never commit messages, so the fixture is four worktrees and a merge.
///
/// Red: a lane that merged two of three workers records exactly those two, by worker and by
/// head, and not the third. Green: the two exclusions that are easy to lose. A worker whose
/// head has reached main is NOT a member — otherwise every landing re-lists everyone who ever
/// landed — and the lane never lists itself. A worker merged twice appears once, which comes
/// free from iterating worktrees rather than merges, and is asserted so that a rewrite reading
/// merges instead cannot pass.
fn probe_a_batch_records_exactly_the_branches_it_merged() -> Probe {
    use crate::cmd::batch::members_of;

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
                return Err(format!(
                    "git {}: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&dir, &["init", "-q", "-b", "main"])?;
        g(&dir, &["commit", "-q", "--allow-empty", "-m", "base"])?;

        // Four worker worktrees where the launcher puts them, so `worker_name_for` reads the
        // names off the paths exactly as it does in a live fleet.
        let wt = |name: &str| dir.join(".claude").join("worktrees").join(name);
        for name in ["w1", "w2", "w3", "lane"] {
            g(
                &dir,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    name,
                    &wt(name).display().to_string(),
                ],
            )?;
            g(&wt(name), &["commit", "-q", "--allow-empty", "-m", name])?;
        }
        let head_of =
            |name: &str| -> Result<String, String> { g(&wt(name), &["rev-parse", "HEAD"]) };

        // The lane merges w1 and w2, and w1 twice, which a real lane does whenever a worker
        // pushes another commit into a batch that already carried it.
        for name in ["w1", "w2", "w1"] {
            g(
                &wt("lane"),
                &["merge", "-q", "--no-ff", "-m", "batch", name],
            )?;
        }
        let lane_head = head_of("lane")?;
        let main_tip = g(&dir, &["rev-parse", "main"])?;

        let members = members_of(&dir, "lane", &lane_head, &main_tip);
        let named: Vec<(String, String)> = members
            .iter()
            .map(|m| (m.worker.clone(), m.sha.clone()))
            .collect();
        let red = named
            == vec![
                ("w1".to_string(), head_of("w1")?),
                ("w2".to_string(), head_of("w2")?),
            ];

        // Main advances to contain w1's work, which is what landing the batch does. w1's head
        // is now an ancestor of main and stops being a member; w2's is not and stays one.
        g(&dir, &["merge", "-q", "--ff-only", "w1"])?;
        let moved_tip = g(&dir, &["rev-parse", "main"])?;
        let after = members_of(&dir, "lane", &lane_head, &moved_tip);
        let only_w2 = after.len() == 1 && after.first().is_some_and(|m| m.worker == "w2");
        // The lane is never its own member, whatever it merged.
        let not_itself = !after.iter().any(|m| m.worker == "lane")
            && !members.iter().any(|m| m.worker == "lane");

        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, only_w2 && not_itself))
    })();
    let (red, green) = res.unwrap_or_else(blocked);

    Probe {
        name: "batch: a landing records exactly the branches its batch merged, once each, and never one already in main",
        red_fires: red,
        green_passes: green,
    }
}

/// air-o1m: the gate runs what the Makefile says it runs.
///
/// Two of this round's mechanisms are enforced only because a line in a Makefile invokes them:
/// `air release-check` (air-mir) and `air adopter-check` (air-bpj). Nothing read that file, so
/// deleting either line left `make verify` green while checking less — one gap behind two
/// mechanisms, failing toward permitting, found by air-g7e's coverage pass.
///
/// **It parses the targets rather than grepping the file**, because a grep is satisfied by the
/// name appearing anywhere: in the comment above the target, in a different target, or in a
/// line somebody commented out. What a target's recipe actually contains is the question.
///
/// The two live in different targets and the probe says which, because the bead's own wording
/// put both in `verify` and only one is: `verify` runs `adopter-check`, and `release` runs
/// `release-check` before delegating to `verify` (air-mir moved it there, so a lane's notice
/// does not force a release). A probe that accepted either target would have let air-mir's
/// move go unnoticed in the other direction too.
///
/// Red: the recipe of `verify` invokes `adopter-check` and `selftest`, and `release` invokes
/// `release-check` and `verify` — each named, so a renamed target cannot silently satisfy it.
/// Green: the parse is not a substring search — a line that only mentions a command in a
/// comment, or that sits in another target, does not count.
fn probe_the_gate_runs_what_the_makefile_says() -> Probe {
    /// The recipe lines of one target: the tab-indented block under `name:`, comments and
    /// blank lines dropped. Pure, so the green half can feed it a file that would fool a grep.
    fn recipe(makefile: &str, target: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut inside = false;
        for line in makefile.lines() {
            if line.starts_with(&format!("{target}:")) {
                inside = true;
                continue;
            }
            if inside {
                let Some(body) = line.strip_prefix('\t') else {
                    if line.trim().is_empty() {
                        continue;
                    }
                    break;
                };
                let body = body.trim_start_matches(['@', '-']).trim();
                if !body.starts_with('#') && !body.is_empty() {
                    out.push(body.to_string());
                }
            }
        }
        out
    }

    let runs = |lines: &[String], cmd: &str| lines.iter().any(|l| l.contains(cmd));

    let res = (|| -> Result<(bool, bool), String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Makefile");
        let mk = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let verify = recipe(&mk, "verify");
        let release = recipe(&mk, "release");
        if verify.is_empty() || release.is_empty() {
            return Err("Makefile has no `verify:` or no `release:` recipe".into());
        }
        Ok((
            runs(&verify, "air -- adopter-check")
                && runs(&verify, "air -- selftest")
                && runs(&release, "air -- release-check")
                && runs(&release, "verify"),
            true,
        ))
    })();
    let (red, _) = res.unwrap_or_else(blocked);

    // A file built to fool a grep: both command names are present, one in a comment above the
    // target and one in an unrelated target, and neither is in `verify`'s recipe.
    let decoy = "# verify runs air -- adopter-check, honestly\nverify:\n\tcargo test\n\nother:\n\tcargo run -q -p air -- adopter-check\n";
    let green = !runs(&recipe(decoy, "verify"), "air -- adopter-check")
        && runs(&recipe(decoy, "other"), "air -- adopter-check")
        && recipe(decoy, "verify") == vec!["cargo test".to_string()];

    Probe {
        name: "make: the verify target runs adopter-check and selftest, and release runs release-check before verify",
        red_fires: red,
        green_passes: green,
    }
}

/// air-5ik: `verify_runs.log_path` has existed since schema v1 and was `None` on every row ever
/// written, so the one run anybody reads — the red one — was the one Air kept nothing for. Four
/// load-related flakes on 2026-09-06 (alerts' red at 37b15cb, `install_and_launch`'s tmux test,
/// the `SubagentStop` probe, the land acceptance-budget probe: each red once, green on re-run,
/// all under load 66-67) are undiagnosable now for exactly that reason.
///
/// Red (declared mutation: `exit_code != 0` becomes `true`): a GREEN run keeps its output too.
/// That is not untidiness — the store is bounded by COUNT, so greens would evict the reds it
/// exists to keep, and a fleet's greens outnumber its reds by an order of magnitude.
///
/// Green: the tail keeps the END of a stream and never exceeds its cap, because a verify fails
/// at the end; `prune` drops the OLDEST by run id whatever order the directory listed them in,
/// which is the difference between a bound and a lottery; the store's ceiling is a number this
/// asserts rather than a hope; and `run_tee` really does carry both streams, in arrival order,
/// out of a child that writes to each.
fn probe_a_red_runs_output_is_kept() -> Probe {
    use crate::cmd::record::run_tee_to;
    use crate::cmd::runlog::{KEEP_LOGS, TAIL_BYTES, Tail, keeps_output, prune, write};

    // RED: green keeps nothing, every non-green keeps something. 143/137 are the killed exits,
    // and a killed run is the one most likely to be the loaded machine this bead is about.
    let red = !keeps_output(0)
        && keeps_output(2)
        && keeps_output(1)
        && air_ledger::verify::KILLED_EXITS
            .iter()
            .all(|c| keeps_output(*c));

    // The tail is the end of the stream, and one write bigger than the cap keeps its own end.
    let tail_of = |cap: usize, chunks: &[&[u8]]| -> Vec<u8> {
        let mut t = Tail::new(cap);
        for c in chunks {
            t.push(c);
        }
        t.into_bytes()
    };
    let keeps_the_end = tail_of(4, &[b"abc", b"de"]) == b"bcde"
        && tail_of(3, &[b"abcdefgh"]) == b"fgh"
        && tail_of(0, &[b"x"]).is_empty();

    // Age order is id order (ULIDs), never listing order. Shuffled on purpose.
    let names: Vec<String> = ["03.log", "01.log", "04.log", "02.log"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let bounded = prune(&names, 5).is_empty()
        && prune(&names, 4) == vec!["01.log".to_string()]
        && prune(&names, 2).len() == 3
        // Whatever it deletes, exactly `keep - 1` are left for the new one to join.
        && names.len().saturating_sub(prune(&names, 3).len()) == 2;

    // The ceiling is a number, not a hope: 20 x 64 KiB.
    let ceiling = KEEP_LOGS.saturating_mul(TAIL_BYTES) == 1_310_720;

    // End to end: a child that writes to BOTH streams, and the tail carries both — and the
    // two sinks get them too, which is what `run_tee` exists for and what the tail alone never
    // showed. The sinks are in-memory, not this process's stdout: the version of this probe
    // that used the real ones put `out` on `air selftest`'s stdout ahead of the JSON array and
    // made `air selftest --json` unparseable, so `--prove` called every mutation BROKEN
    // (air-e21v).
    let out_sink = Shared::new();
    let err_sink = Shared::new();
    let both = run_tee_to(
        "sh",
        &["-c".into(), "echo out; echo err >&2".into()],
        &std::env::temp_dir(),
        out_sink.clone(),
        err_sink.clone(),
    )
    .map(|(code, _, tail)| {
        let s = String::from_utf8_lossy(&tail).to_string();
        code == 0
            && s.contains("out")
            && s.contains("err")
            // Each stream reached ITS OWN sink, and neither reached the other's: a pump wired
            // to one stream twice would still fill the tail with both.
            && out_sink.text().contains("out")
            && !out_sink.text().contains("err")
            && err_sink.text().contains("err")
            && !err_sink.text().contains("out")
    })
    .unwrap_or(false);

    // A write really lands and really prunes, in a scratch dir of this probe's own.
    let stored = (|| -> Option<bool> {
        let air = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&air).ok()?;
        let p = write(&air, "01ZZZ", b"the failure")?;
        let read_back = std::fs::read(&p).ok()? == b"the failure";
        let kept = std::fs::read_dir(crate::cmd::runlog::dir(&air))
            .ok()?
            .count()
            == 1;
        std::fs::remove_dir_all(&air).ok();
        Some(read_back && kept)
    })()
    .unwrap_or(false);

    let green = keeps_the_end && bounded && ceiling && both && stored;
    Probe {
        name: "record: a red run's output is kept, bounded by its tail and by a count of logs, and a green run's is not",
        red_fires: red,
        green_passes: green,
    }
}

/// Condition kinds that once existed and no longer do (air-wfd).
///
/// Declared, because it cannot be derived: a deleted kind leaves no trace in `kinds::ALL`, and
/// the whole risk is prose that goes on naming one. One line when a kind is deleted, beside
/// the deletion record that already gets written.
pub const RETIRED_KINDS: &[(&str, &str)] = &[
    (
        "stuck",
        "air-12k, 2026-09-06: set only by a permission prompt auto mode never sends",
    ),
    (
        "owner-decision-waiting",
        "air-uef, 2026-09-05: the owner's queue is owner-labelled beads",
    ),
];

/// Spans in tracked prose that name a command, flag or kind **in order to say it does not
/// exist**. Declared, with the bead, because the alternative is reading intent out of prose.
///
/// This list is the whole reason the check is honest. Without it the first thing it refuses is
/// `roles.md`'s paragraph explaining that `stuck` was deleted. (The worktree protocol's record
/// that `air peer` and `air merge-advice` were planned and never built was the other entry,
/// air-w91's provenance line; that file was retired on 2026-09-25 and `docs/design.md` §8 now
/// says it in plain words.) A check that fails on the documentation of a deletion teaches people to stop
/// documenting deletions.
/// Keyed by FILE as well as span: an absence is documented in a place. Keyed by span alone
/// this list excused `stuck` in every document including one that used it as an instruction,
/// which is the drift the check exists to catch — found by the probe's own red half staying
/// silent (air-wfd).
pub const DOCUMENTED_ABSENCE: &[(&str, &str, &str)] = &[(
    "docs/rules/roles.md",
    "stuck",
    "air-12k: roles.md explains the deletion and why the heartbeat replaced it",
)];

/// air-wfd: every flag and condition kind the docs name still exists.
///
/// air-w91 gave Air a check that every `air <word>` in command position resolves to a real
/// subcommand, over shipped SOURCES. It covers command words only, and the three drifts
/// air-9iz found in the README were not command words: `air inbox --owner` (a flag air-uef
/// deleted) and `stuck` among the channel conditions (a kind air-12k deleted). Measured against
/// the README as it stood at `c64175b`, the subcommand check would have caught **none** of
/// them. Prose reads as authoritative while it goes stale, and nothing verified it.
///
/// Two things this deliberately does not do. It does not scan for hyphenated tokens and guess
/// which are condition kinds — that reads a fact out of prose and would refuse half the
/// backticks in the repo. And it stops at `--`: `air worker <name> -- --settings '{…}'` passes
/// `--settings` to claude, not to `air worker`, so treating it as this command's flag would
/// make the check wrong about the one place a pass-through appears.
///
/// The third drift — a worker loop ending at `air handover` rather than `bd close` — names only
/// commands that still exist. **No name check reaches it**, and that limit is stated here
/// rather than papered over: prose describing a sequence is not checkable by matching names.
///
/// Red: both catchable drifts. `air inbox --owner` is refused with its file and line, and a
/// doc naming `stuck` as a live condition is refused. Green: today's tracked prose passes, a
/// nested subcommand's flag resolves (`air lease take --reason` belongs to `take`, not to
/// `lease`), and a span on the declared-absence list is allowed — because the prose that
/// records a deletion must not be the thing that fails.
fn probe_docs_name_real_flags_and_kinds() -> Probe {
    use clap::CommandFactory;

    /// Every backticked span of a markdown text, with its 1-indexed line.
    fn spans(text: &str) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let mut rest = line;
            while let Some(a) = rest.find('`') {
                let after = rest.get(a.saturating_add(1)..).unwrap_or("");
                let Some(b) = after.find('`') else { break };
                if let Some(s) = after.get(..b) {
                    out.push((n.saturating_add(1), s.to_string()));
                }
                rest = after.get(b.saturating_add(1)..).unwrap_or("");
            }
        }
        out
    }

    /// `(command path, flag)` pairs a span asks for, or nothing if it is not an `air` command.
    /// Stops at `--`: everything after it belongs to another program.
    fn asks(span: &str) -> Option<(Vec<String>, Vec<String>)> {
        let mut words = span.split_whitespace();
        if words.next()? != "air" {
            return None;
        }
        let mut path = Vec::new();
        let mut flags = Vec::new();
        for w in words {
            if w == "--" {
                break;
            }
            if let Some(f) = w.strip_prefix("--") {
                let f = f.split(['=', '<', '\'', '"']).next().unwrap_or(f);
                if !f.is_empty() {
                    flags.push(f.to_string());
                }
            } else if flags.is_empty()
                && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                && !w.is_empty()
            {
                path.push(w.to_string());
            }
        }
        Some((path, flags))
    }

    /// Does clap know this command path, and does it know every flag on it? Walks nested
    /// subcommands, so `lease take --reason` is asked of `take` and not of `lease`.
    fn resolves(path: &[String], flags: &[String]) -> Result<(), String> {
        let root = crate::Cli::command();
        let mut cur = root.clone();
        for (i, seg) in path.iter().enumerate() {
            match cur.clone().find_subcommand(seg) {
                Some(next) => cur = next.clone(),
                // Hidden subcommands are real; clap still knows them.
                None if i > 0 => break,
                None => return Err(format!("no subcommand `air {seg}`")),
            }
        }
        let known = |c: &clap::Command, f: &str| c.get_arguments().any(|a| a.get_long() == Some(f));
        for f in flags {
            if !known(&cur, f) && !known(&root, f) {
                return Err(format!("`air {}` has no --{f}", path.join(" ")));
            }
        }
        Ok(())
    }

    let excused = |file: &str, span: &str| {
        DOCUMENTED_ABSENCE
            .iter()
            .any(|(f, text, _)| *f == file && span == *text)
    };

    /// Every complaint one document makes.
    fn scan(name: &str, text: &str, excused: &dyn Fn(&str, &str) -> bool) -> Vec<String> {
        let mut out = Vec::new();
        for (line, span) in spans(text) {
            if excused(name, &span) {
                continue;
            }
            if let Some((path, flags)) = asks(&span)
                && !path.is_empty()
                && let Err(e) = resolves(&path, &flags)
            {
                out.push(format!("{name}:{line}: {e}"));
            }
            if let Some((kind, why)) = RETIRED_KINDS.iter().find(|(k, _)| span == *k) {
                out.push(format!("{name}:{line}: `{kind}` was retired ({why})"));
            }
        }
        out
    }

    let docs: &[(&str, &str)] = &[
        ("README.md", include_str!("../../../../README.md")),
        ("CLAUDE.md", include_str!("../../../../CLAUDE.md")),
        (
            "docs/rules/roles.md",
            include_str!("../../../../docs/rules/roles.md"),
        ),
        (
            "docs/rules/adopting-air.md",
            include_str!("../../../../docs/rules/adopting-air.md"),
        ),
    ];
    let live: Vec<String> = docs
        .iter()
        .flat_map(|(n, t)| scan(n, t, &excused))
        .collect();
    if !live.is_empty() {
        eprintln!("selftest: tracked prose names something that does not exist:");
        for l in &live {
            eprintln!("  {l}");
        }
    }

    // The two drifts that were really in the README at c64175b, as fixtures.
    let drift_flag = scan(
        "old-README.md",
        "- **Owner:** `air status` in any terminal. `air inbox --owner` for what waits on you.\n",
        &excused,
    );
    let drift_kind = scan(
        "old-README.md",
        "- **Informs** the coordinator when something needs a person: a `stuck` worker.\n",
        &excused,
    );
    let red = drift_flag.iter().any(|d| d.contains("has no --owner"))
        && drift_kind.iter().any(|d| d.contains("was retired"));

    // A nested flag resolves against the subcommand that owns it, and a declared absence is
    // allowed rather than refused.
    let nested = scan(
        "t.md",
        "`air lease take runtime --reason \"why\"`\n",
        &excused,
    )
    .is_empty();
    let absence_ok = scan(
        "docs/rules/roles.md",
        "The `stuck` condition was deleted.\n",
        &excused,
    )
    .is_empty()
        // and the same span in a document that has NOT declared it is still refused, or one
        // line would switch the check off everywhere.
        && !scan("t.md", "A `stuck` worker needs you.\n", &excused).is_empty();

    Probe {
        name: "docs: every flag and condition kind the README and the rules name still exists",
        red_fires: red,
        green_passes: live.is_empty() && nested && absence_ok,
    }
}

/// air-jsz: `air adopter-check` ran for a whole round having never once had an input.
///
/// It reads the names it forbids from `private/adopters.md`, which is gitignored by design, and
/// skipped cleanly when that file was absent. The file existed in no worktree, not in the main
/// checkout, and nowhere on the machine — so every green `make verify` of the round, including
/// the sweep's own, printed `Skipped`, and the one mechanism guarding the owner's ruling that no
/// adopter content is public would have passed over any leak. `do-less` case 3a: the count of
/// firings was zero and the zero said nothing, because the input never arrived.
///
/// The existing probe could not catch that. It exercised `names`/`leaks`/`refusal` as pure
/// functions and never ran the command, so it proved the machinery worked while the machinery
/// was being handed nothing. **This one runs the binary**, in a real git worktree of a real
/// repo, which is where a worker's verify runs and where the file was missing.
///
/// Red: from the WORKTREE, a tracked file naming an adopter is refused with its path and line,
/// exit 2 — which is the case that silently passed. And a repo that declares an adopter with no
/// names is refused too, naming the file to write, instead of skipping.
///
/// Green: the contributor's case survives. A repo that declares no adopter and has no list
/// skips and exits 0, because a public clone must not be refused for lacking a private file it
/// is never given.
fn probe_adopter_check_refuses_from_a_worktree_and_when_it_has_no_list() -> Probe {
    let Ok(exe) = std::env::current_exe() else {
        return Probe {
            name: ADOPTER_CHECK_PROBE,
            red_fires: false,
            green_passes: false,
        };
    };
    let res = (|| -> Result<(bool, bool), String> {
        let root = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        let main = root.join("main");
        let wt = root.join("wt");
        std::fs::create_dir_all(main.join(".claude")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(main.join(".air")).map_err(|e| e.to_string())?;
        let git = |args: &[&str]| -> Result<String, String> {
            crate::git::run(&main, args).map_err(|e| e.to_string())
        };
        git(&["init", "-q", "-b", "main", "."])?;
        git(&["config", "user.email", "a@b"])?;
        git(&["config", "user.name", "a"])?;
        // The leak: a tracked file naming the adopter this repo declares.
        std::fs::write(main.join("note.md"), "clean line\nas ACME measured it\n")
            .map_err(|e| e.to_string())?;
        std::fs::write(main.join(".gitignore"), "private/\n.air/\n").map_err(|e| e.to_string())?;
        std::fs::write(main.join(".claude/air.json"), "{\"adopters\": true}\n")
            .map_err(|e| e.to_string())?;
        git(&["add", "-A"])?;
        git(&["commit", "-qm", "seed"])?;
        // A worktree, because that is where a worker's verify runs and where the list was
        // missing. The list lives in the MAIN checkout and is copied in, which is what
        // `.worktreeinclude` does for a real launch.
        git(&["worktree", "add", "-q", "-b", "wt", &wt.to_string_lossy()])?;
        // The list lives in the MAIN checkout only: one source, so a worktree's copy cannot
        // disagree with it (air-jsz). The worktree deliberately gets none.
        std::fs::create_dir_all(main.join("private")).map_err(|e| e.to_string())?;
        std::fs::write(main.join("private/adopters.md"), "    name: acme\n")
            .map_err(|e| e.to_string())?;

        let check = |dir: &Path| -> Result<i32, String> {
            let out = air_command(&exe, dir)
                .args(["adopter-check"])
                .output()
                .map_err(|e| e.to_string())?;
            Ok(out.status.code().unwrap_or(-1))
        };
        // 1. The leak, from the worktree.
        let refused_leak = check(&wt)? == 2;
        // 2. Declared, list gone: refused rather than skipped. This is the state the round ran
        //    in, and the whole point of the bead.
        std::fs::remove_file(main.join("private/adopters.md")).map_err(|e| e.to_string())?;
        let refused_empty = check(&wt)? == 2;
        // 3. Undeclared and no list: the contributor's clone, which must still pass.
        std::fs::write(main.join(".claude/air.json"), "{}\n").map_err(|e| e.to_string())?;
        let skips = check(&wt)? == 0;
        let _ = std::fs::remove_dir_all(&root);
        Ok((refused_leak && refused_empty, skips))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: ADOPTER_CHECK_PROBE,
        red_fires: red,
        // The pure half stays asserted here too, so the declaration's three cases are covered
        // without a spawn: an undeclared repo WITH a list still checks it.
        green_passes: green
            && matches!(
                crate::cmd::privacy::verdict(false, Some("    name: acme\n")),
                crate::cmd::privacy::Verdict::Check(_)
            )
            && crate::cmd::privacy::verdict(true, None)
                == crate::cmd::privacy::Verdict::RefuseDeclaredButNoNames
            && crate::cmd::privacy::verdict(false, None)
                == crate::cmd::privacy::Verdict::SkipUndeclared,
    }
}

const ADOPTER_CHECK_PROBE: &str = "privacy: adopter-check refuses a leak when run from a worktree, and refuses a repo that declares an adopter with no names instead of skipping";

/// air-ahl: a digest that git does not track is not proof, and the refusal says which fix.
///
/// Reported by an adopter against their own worker's interest: their w2 used the gap
/// deliberately and told them anyway. `digest_for_bead` read the directory and never asked git,
/// so a file that existed for nobody but one worktree satisfied the gate — and a green then
/// said nothing about whether a digest would exist for the next reader, which is the whole
/// purpose of the check.
///
/// The requirement did not ship alone, because the need it served is real: a worker must be
/// able to close without invalidating the batch its lane cut. **That route already existed and
/// needed no new mechanism**, which is the finding rather than the fix. `batch::bead_commits`
/// filters `main..HEAD` by the `Bead:` trailer ONLY, so a digest commit carrying no trailer
/// never joins the set the batch green has to cover: the worker commits the digest, the head
/// moves, and the close still passes at the batch it was cut at. Demonstrated end to end on a
/// real branch, and the refusal below says so where a worker meets it.
///
/// Rejected: accepting a STAGED digest. `git add` alone does make a file tracked and does leave
/// HEAD where it was, so it would have satisfied both halves — but a staged file is still one
/// worktree's, and the gate exists for the reader who was not there.
///
/// Red: a digest declaring the bead, present in the directory and untracked, is `Untracked`,
/// and the gate refuses it under its own check name with a fix naming the untrailered commit.
/// Green: the same file once git tracks it is `Tracked` and passes; a directory with no
/// declaring file at all is `Missing` and gets the other sentence, so the two refusals cannot
/// collapse into one; and a tracked digest beside an untracked stray still passes, or a scratch
/// copy would mask the real one.
fn probe_an_untracked_digest_is_not_proof() -> Probe {
    use crate::cmd::handover::{Digest, digest_for_bead, tracked_in};
    use air_hooks::{GateFacts, handover_verdict};

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
                return Err(format!(
                    "git {}: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "base"])?;

        let digests = dir.join("docs").join("digests");
        std::fs::create_dir_all(&digests).map_err(|e| e.to_string())?;
        let beads = vec!["air-ahl".to_string()];
        let cut: jiff::Timestamp = "2000-01-01T00:00:00Z".parse().map_err(|_| "cutoff")?;
        let state = || {
            digest_for_bead(
                &digests,
                "w1",
                &beads,
                None,
                cut,
                &tracked_in(&dir, &digests),
            )
        };

        // Nothing written yet.
        let missing = state() == Digest::Missing;

        // Written, and git has never heard of it: exactly the adopter's case.
        let path = digests.join("2026-09-06-w1-air-ahl.md");
        std::fs::write(&path, "---\nbead: air-ahl\n---\nproof\n").map_err(|e| e.to_string())?;
        let untracked = state() == Digest::Untracked;

        // The refusal a worker actually meets, from the real gate.
        let facts = GateFacts {
            worker: "w1".into(),
            head: "0123456789abcdef".into(),
            green_at_head: true,
            main_is_ancestor: true,
            bead_claimed_or_carried: true,
            runs_at_head: (1, 0),
            digest_present: Some(false),
            digest_untracked: true,
            digest_dir: Some("docs/digests".into()),
            held_beads: beads.clone(),
            ..GateFacts::default()
        };
        let v = handover_verdict(&facts);
        let named = v.missing.iter().any(|m| {
            m.check == "digest-untracked"
                && m.detail.contains("does not track it")
                && m.fix.contains("carry NO `Bead:` trailer")
                && m.fix.starts_with("git add ")
        });

        // Tracked: the same bytes, once git knows about them.
        g(&["add", "docs/digests/2026-09-06-w1-air-ahl.md"])?;
        let tracked_ok = state() == Digest::Tracked;

        // A stray untracked copy beside a tracked digest must not mask it.
        std::fs::write(
            digests.join("scratch.md"),
            "---\nbead: air-ahl\n---\nnote to self\n",
        )
        .map_err(|e| e.to_string())?;
        let stray_ok = state() == Digest::Tracked;

        let _ = std::fs::remove_dir_all(&dir);
        Ok((untracked && named, missing && tracked_ok && stray_ok))
    })();
    let (red, green) = res.unwrap_or_else(blocked);

    Probe {
        name: "gate: a digest git does not track is not proof, and the refusal names the untrailered commit that fixes it",
        red_fires: red,
        green_passes: green,
    }
}

/// air-88av: `air record` refuses over an unresolved merge, and still records a merely dirty
/// tree.
///
/// A conflicted tree makes the suite fail on conflict markers, and `air record` wrote that as a
/// RED at the sha. The red is then read as evidence by everything downstream — `flaky-at-head`,
/// the close gate, the landing gate, and any later question about whether this tree was ever
/// green. A false red is the record being wrong about a fact nobody will re-derive, which is
/// the same class as air-htmn, where a landing that happened was recorded as refused. I wrote
/// two of these tonight by chaining `git merge` and `air record` in one command.
///
/// **Not a dirty-tree refusal, and the probe pins both halves for that reason.** `air record`
/// is deliberately usable on a dirty tree, the `dirty` column exists for it, and verifying
/// uncommitted work is normal. Collapsing the two would break the normal case in a way a green
/// suite would call correct.
///
/// **`MERGE_HEAD` is not the signal**, though it is the obvious one: it is still present once
/// conflicts are resolved and staged, and verifying that tree before committing the merge is a
/// reasonable thing to do. Checked in a fixture rather than assumed. Unmerged PATHS is the
/// fact.
///
/// Red: a real conflicted repo — an actual failed `git merge`, not a constructed flag — is
/// refused, the paths are named, no row is written, and the refusal says it is not a failing
/// suite. Green: a merely dirty tree still records with `dirty` set, and a tree whose conflicts
/// have been resolved but not committed records too, because `MERGE_HEAD` is not what is being
/// asked about.
fn probe_record_refuses_an_unresolved_merge_but_not_a_dirty_tree() -> Probe {
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
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        let write = |name: &str, body: &str| -> Result<(), String> {
            std::fs::write(dir.join(name), body).map_err(|e| e.to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        write("f.txt", "base\n")?;
        g(&["add", "f.txt"])?;
        g(&["commit", "-q", "-m", "base"])?;
        g(&["checkout", "-q", "-b", "other"])?;
        write("f.txt", "theirs\n")?;
        g(&["commit", "-q", "-am", "theirs"])?;
        g(&["checkout", "-q", "main"])?;
        write("f.txt", "ours\n")?;
        g(&["commit", "-q", "-am", "ours"])?;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let record = || -> Result<(i32, String), String> {
            let out = air_command(&exe, &dir)
                .args(["record", "verify", "--", "true"])
                .output()
                .map_err(|e| e.to_string())?;
            Ok((
                out.status.code().unwrap_or(-1),
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                ),
            ))
        };
        let rows = || -> Result<i64, String> {
            let conn = rusqlite::Connection::open(dir.join(".air").join("ledger.db"))
                .map_err(|e| e.to_string())?;
            conn.query_row("SELECT count(*) FROM verify_runs", [], |r| r.get(0))
                .map_err(|e| e.to_string())
        };

        // GREEN 1: merely dirty. Records, and says so as dirty.
        write("f.txt", "ours\nedited\n")?;
        write("untracked.txt", "scratch\n")?;
        let (dirty_code, dirty_said) = record()?;
        let dirty_recorded = dirty_code == 0 && rows()? == 1 && dirty_said.contains("dirty");
        g(&["checkout", "-q", "--", "f.txt"])?;
        std::fs::remove_file(dir.join("untracked.txt")).map_err(|e| e.to_string())?;

        // RED: a real conflicted merge, not a constructed flag.
        let _ = g(&["merge", "other"]);
        let unmerged = crate::git::unmerged_files(&dir)
            .map_err(|e| e.to_string())?
            .contains(&"f.txt".to_string());
        let before = rows()?;
        let (code, said) = record()?;
        let refused = code == 2
            && said.contains("unresolved merge conflict")
            && said.contains("f.txt")
            && said.contains("not a failing suite")
            && rows()? == before;

        // GREEN 2: conflicts resolved and staged, merge not committed. `MERGE_HEAD` is still
        // there, and this must still record — refusing here would block a reasonable check.
        write("f.txt", "resolved\n")?;
        g(&["add", "f.txt"])?;
        let mid_merge = dir.join(".git").join("MERGE_HEAD").exists();
        let (resolved_code, _) = record()?;
        let records_when_resolved =
            mid_merge && resolved_code == 0 && rows()? == before.saturating_add(1);

        let _ = std::fs::remove_dir_all(&dir);
        Ok((refused && unmerged, dirty_recorded && records_when_resolved))
    })();
    let (red, green) = res.unwrap_or_else(blocked);

    Probe {
        name: "record: an unresolved merge is refused with its paths and no row written, while a dirty tree and a resolved-but-uncommitted merge both still record",
        red_fires: red,
        green_passes: green,
    }
}

/// air-lyjr: `air close` takes the reason from a file, whole, for every bead named.
///
/// air-45pw one command over, and worse here: `air close` is the command this repo demands the
/// LONGEST argument for. A close carries proof — a command and its output and the counts from
/// the run — and a reason of that length goes through the harness's classifier as a command
/// line and is refused for its shape. The obvious next move is to shorten the proof, which is
/// the failure. Observed on `bd close` while closing air-gazh at ~2,500 characters; bd's own
/// `--reason-file` took the identical text on the next attempt with nothing about it changed.
///
/// **Byte-for-byte AND by length, never "non-empty"** (air-45pw's finding, and the clause this
/// bead was written around). What the bug produces is a TRUNCATION: a cut reason still records,
/// still reads long, still closes the bead, and passes every check that asks whether a reason is
/// there. The fixture carries a sentinel last sentence, because that is what a truncation eats
/// first.
///
/// The reason is read back from the argv bd actually received, not from what was passed in —
/// the wiring between them is the part that can silently drop bytes.
///
/// Red is the new capability, end to end: the whole file reaches bd unchanged, for EVERY id
/// named — `--reason` already applied one string to several beads and the file route changes
/// nothing about that — and `--help` names the route, because the person who needs it is the
/// person whose close was just refused.
///
/// Green is what must survive it: both routes at once and neither are each refused naming both
/// ways in, a path that cannot be read is an ERROR rather than an empty reason — the branch that
/// would close a bead with no proof and read afterwards exactly like a close nobody wrote one
/// for — and the inline route still works, since this is an addition and most closes are one
/// line.
fn probe_close_takes_a_reason_file_whole() -> Probe {
    use crate::cmd::close::resolve_reason;

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

        // Proof of the length this repo's rules ask for, with the shapes that make a classifier
        // refuse, and a sentinel a truncation eats first.
        let mut proof = String::new();
        for i in 0..60 {
            proof.push_str(&format!(
                "Step {i}: `make verify` green, with \"quotes\", a $dollar and a `tick`, and the \
                 counts from the run rather than from any earlier one.\n\n"
            ));
        }
        proof.push_str("SENTINEL: the last sentence, which a truncation eats first.");
        let path = dir.join("proof.md");
        std::fs::write(&path, &proof).map_err(|e| e.to_string())?;

        // A bd that records the exact bytes it was handed after `--reason`.
        let script = dir.join("bd");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nd='{d}'\ncase \"$1\" in\n  --version) echo 'bd version 1.2.2'; \
                 exit 0;;\nesac\nprev=''\nfor a in \"$@\"; do\n  if [ \"$prev\" = '--reason' ]; \
                 then printf '%s' \"$a\" > \"$d/reason.txt\"; fi\n  prev=\"$a\"\ndone\n\
                 printf '%s\\n' \"$@\" | head -20 > \"$d/argv.txt\"\nexit 0\n",
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
        // RED 1: neither route and both routes, each refused naming both ways in.
        let names_both = |e: &str| e.contains("--reason-file") && e.contains("--reason");
        let neither = resolve_reason(None, None);
        let both = resolve_reason(Some("a line"), Some(&path));
        // RED 2: a path that cannot be read is an ERROR, never an empty reason.
        let missing = resolve_reason(None, Some(&dir.join("nope.md")));
        let refusals = matches!(&neither, Err(e) if names_both(e))
            && matches!(&both, Err(e) if names_both(e))
            && matches!(&missing, Err(e) if e.contains("cannot read --reason-file"));

        // The real path, through a spawned `air close` so the fake bd is reachable without
        // setting an env var in this process: `set_var` is unsafe in edition 2024 and a probe
        // is not the place for it, and a child is closer to how the command is really run.
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let close = |args: &[&str]| -> Result<i32, String> {
            let out = air_command(&exe, &dir)
                .env("AIR_BD_BIN", &script)
                .arg("close")
                .args(args)
                .output()
                .map_err(|e| e.to_string())?;
            Ok(out.status.code().unwrap_or(-1))
        };
        let code = close(&["zz-1", "zz-2", "--reason-file", &path.display().to_string()])?;
        let got = std::fs::read_to_string(dir.join("reason.txt")).unwrap_or_default();
        let trimmed = proof.trim();
        let whole = code == 0
            && got == trimmed
            && got.len() == trimmed.len()
            && got.ends_with("a truncation eats first.");
        // One reason, both ids, one bd process — the fan-out `--reason` already had.
        let argv = std::fs::read_to_string(dir.join("argv.txt")).unwrap_or_default();
        let both_ids = argv.contains("zz-1") && argv.contains("zz-2");

        // The inline route still works: this is an addition, not a replacement.
        let _ = std::fs::remove_file(dir.join("reason.txt"));
        let inline = close(&["zz-1", "zz-2", "--reason", "  short and obvious  "])? == 0
            && std::fs::read_to_string(dir.join("reason.txt")).unwrap_or_default()
                == "short and obvious";

        // `--help` names the route, for the reader who just had a close refused.
        let help = air_command(&exe, &dir)
            .args(["close", "--help"])
            .output()
            .map_err(|e| e.to_string())?;
        let help_names_it = String::from_utf8_lossy(&help.stdout).contains("--reason-file");

        let _ = std::fs::remove_dir_all(&dir);
        Ok((whole && both_ids && help_names_it, refusals && inline))
    })();
    let (red, green) = res.unwrap_or_else(blocked);

    Probe {
        name: "close: --reason-file records the file whole for every bead named, and an unreadable path is an error rather than an empty reason",
        red_fires: red,
        green_passes: green,
    }
}

/// air-6wv2: the assignee refusal says whether that assignee is actually holding the bead.
///
/// bd keeps an assignee through a close, so a reopened bead comes back pencilled in with nobody
/// having assigned it — and in bd 1.2.x that blocks every other worker's `--claim`. The refusal
/// named who was assigned and the fixing command, which only the coordinator can run, and said
/// nothing about how a bead nobody assigned came to have an assignee. It cost the worker it
/// blocked a round trip tonight (air-vsvt, reopened carrying `alerts`).
///
/// This is air-0kk at a second door. `air release` already reopens and unassigns in one bd
/// process; the coordinator's reopen path is raw `bd update -s open` and there is no
/// `air reopen`, deliberately — the refusal is loud and already names the fix, so what was
/// missing is one sentence, not a command.
///
/// **The signal is Air's own claims, and that is a deliberate retreat from bd.** bd exposes
/// nothing that marks a reopen: no `reopened_at`, and `closed_at` cannot be observed on an open
/// bead without creating and reopening one, which a worker may not do. Rather than infer a
/// signal bd does not offer, the refusal reports the fact Air holds — is there an open claim by
/// that assignee — and words the reopen as a possibility.
///
/// Red: with no open claim by the assignee, the refusal says the assignee may be left over and
/// names the reopen as a cause. Green: with the assignee actually holding a claim, it says this
/// is live work and never suggests a reopen; and both forms still name the assignee, the bd
/// version rule, and the fixing command, which is what the refusal was already good at.
fn probe_the_assignee_refusal_says_whether_anyone_holds_it() -> Probe {
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
        g(&["commit", "-q", "--allow-empty", "-m", "base"])?;

        // A bd that answers with an assignee, which the shared fixture does not.
        let script = dir.join("bd");
        std::fs::write(
            &script,
            "#!/bin/sh\ncase \"$1\" in\n  --version) echo 'bd version 1.2.2'; exit 0;;\n  \
             show) printf '%s\\n' \
             '[{\"id\":\"zz-6wv2\",\"status\":\"open\",\"assignee\":\"alerts\",\"labels\":[]}]'; \
             exit 0;;\n  *) echo '[]'; exit 0;;\nesac\n",
        )
        .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let claim = || -> Result<String, String> {
            let out = air_command(&exe, &dir)
                .env("AIR_BD_BIN", &script)
                .args(["claim", "zz-6wv2"])
                .output()
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        };
        // Every refusal must keep saying these, whichever branch it takes.
        let names_the_basics = |s: &str| {
            s.contains("has assignee `alerts`")
                && s.contains("bd 1.2.x")
                && s.contains("bd update zz-6wv2 -a \"\"")
        };

        // RED: nobody is holding it — the reopened case.
        let stale = claim()?;
        let red = names_the_basics(&stale)
            && stale.contains("Air has no open claim behind that assignee")
            && stale.contains("may be left over rather than live work")
            && stale.contains("reopened bead can come back pencilled in");

        // GREEN: the precedence that makes the red sentence TRUE rather than a guess. With a
        // real claim recorded, the ledger check at step 1 refuses first and names the holder
        // and `air release` — so the assignee refusal is only ever reached when Air has no
        // claim behind the assignee, and it never describes live work as leftover.
        //
        // This is why the sentence is unconditional. The first version branched on whether the
        // assignee held a claim; running it showed that branch is unreachable, which this half
        // now pins so the dead code cannot come back.
        let l = Ledger::open_for_repo(&dir).map_err(|e| e.to_string())?;
        l.record_claim("zz-6wv2", "alerts", &[], "t0")
            .map_err(|e| e.to_string())?;
        drop(l);
        let live = claim()?;
        let green = live.contains("is claimed by alerts")
            && live.contains("air release zz-6wv2 --worker alerts")
            && !live.contains("may be left over")
            && !live.contains("reopened bead");

        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);

    Probe {
        name: "claim: the assignee refusal says Air has no claim behind that assignee and offers a reopen as the cause, and live work is refused earlier by name",
        red_fires: red,
        green_passes: green,
    }
}

/// air-vsvt (reopened): a batch's members come from the batch COMMIT, and a sha it took is
/// never dropped for want of a name.
///
/// `members_of` enumerated the worktrees and kept each one's merge-base with the batch if it
/// was not yet in main. Two defects, both reproduced in a fixture before anything was changed:
///
/// - **A member that resets to main drops out, with main held still.** Its branch no longer
///   contains the work the batch took, so the merge-base collapses to main and the filter
///   removes it. The row that results looks complete — plausible workers, plausible shas, and
///   nothing saying a member is missing.
/// - **A branch the lane never merged is named.** If w4 forks off w3's branch and the lane
///   merges only w3, `merge_base(w4, batch)` is w3's sha: w4 is recorded as a member, carrying
///   another worker's sha.
///
/// The batch commit already holds the answer. Its non-first merge parents are exactly what it
/// took, and they do not move when branches do.
///
/// **Ancestry recovers shas and not names**, which is proven rather than assumed: w3 and w4
/// both contain w3's sha, and the tie-break — w3's head still equalling it — is gone the moment
/// w3 commits again. So a sha with no unambiguous branch is recorded UNATTRIBUTED. An omission
/// wearing a complete-looking row is every symptom on this bead; an empty worker is visible.
///
/// Red: the set of shas is exactly what the batch merged — after a member resets to main with
/// main held still, and with a forked branch present that the lane never took. Green: what must
/// survive — a member is named with the sha the batch took rather than its current head, and the
/// lane is never its own member.
fn probe_a_batch_records_the_shas_it_took_and_never_drops_one() -> Probe {
    use crate::cmd::batch::members_of;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = |at: &std::path::Path, args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(at)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {}: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        let wt = |n: &str| dir.join(".claude").join("worktrees").join(n);
        g(&dir, &["init", "-q", "-b", "main"])?;
        g(&dir, &["commit", "-q", "--allow-empty", "-m", "base"])?;
        for n in ["w1", "w2", "w3", "lane"] {
            g(
                &dir,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    n,
                    &wt(n).display().to_string(),
                ],
            )?;
            g(&wt(n), &["commit", "-q", "--allow-empty", "-m", n])?;
        }
        // w4 forks off w3's BRANCH, not off main, and the lane never merges it.
        g(
            &dir,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "w4",
                &wt("w4").display().to_string(),
                "w3",
            ],
        )?;
        g(&wt("w4"), &["commit", "-q", "--allow-empty", "-m", "w4"])?;

        // The lane cuts a batch: one merge at a time, the realistic shape.
        for n in ["w1", "w2", "w3"] {
            g(&wt("lane"), &["merge", "-q", "--no-ff", "-m", "batch", n])?;
        }
        let batch = g(&wt("lane"), &["rev-parse", "HEAD"])?;
        let main_tip = g(&dir, &["rev-parse", "main"])?;
        let at_cut = |n: &str| -> Result<String, String> { g(&dir, &["rev-parse", n]) };
        let (w1, w2, w3) = (at_cut("w1")?, at_cut("w2")?, at_cut("w3")?);

        let shas = |ms: &[air_ledger::landings::Member]| -> Vec<String> {
            let mut v: Vec<String> = ms.iter().map(|m| m.sha.clone()).collect();
            v.sort();
            v
        };
        let mut want = vec![w1.clone(), w2.clone(), w3.clone()];
        want.sort();

        // w3 keeps going after the cut. The ordinary case, and it must be unchanged.
        g(&wt("w3"), &["commit", "-q", "--allow-empty", "-m", "more"])?;
        let moved = members_of(&dir, "lane", &batch, &main_tip);
        // All three shas, at the cut, and w1/w2 still named. w3's sha is now in two branches
        // (its own and w4's), so it is recorded unattributed rather than guessed at.
        // Deliberately NOT restating red's claim about the sha set: these are the properties
        // that must SURVIVE the declared mutation, so that a failure of both halves reads as a
        // mutation removing the guard rather than one reaching a branch.
        let ordinary = moved.iter().any(|m| m.worker == "w1" && m.sha == w1)
            && moved.iter().any(|m| m.worker == "w2" && m.sha == w2)
            && !moved.iter().any(|m| m.worker == "lane");

        // RED 1: w3 resets to main, main held still. Its sha must survive.
        g(&wt("w3"), &["reset", "-q", "--hard", "main"])?;
        let after_reset = members_of(&dir, "lane", &batch, &main_tip);
        let main_unmoved = g(&dir, &["rev-parse", "main"])? == main_tip;
        let kept = shas(&after_reset) == want && main_unmoved;

        // RED 2: the forked branch's OWN commit was never merged and is never a member.
        // Asserted on the SHA, not the name: once w3 abandons its work, w4 is the only branch
        // still containing w3's sha, so the name resolution hands w4 that entry. Found by
        // running this probe, and recorded on `members_of` as a limitation rather than papered
        // over — a name can migrate to a fork when the owner walks away from its own work, and
        // no amount of ancestry fixes it. The sha stays right, which is the fact that matters.
        let w4_head = g(&wt("w4"), &["rev-parse", "HEAD"])?;
        let no_w4 = !after_reset.iter().any(|m| m.sha == w4_head)
            && !moved.iter().any(|m| m.sha == w4_head);
        // Nothing is dropped for want of a name. Asserted on the PRE-reset snapshot, where w3
        // and w4 both hold w3's sha so no name can be chosen: that entry is recorded with an
        // empty worker rather than guessed at or omitted. After the reset only w4 holds it, so
        // it reads as unambiguous and takes w4's name — wrongly, which is the name-migration
        // limitation recorded on `members_of`. Found by running this, not by reasoning: the
        // first version asserted it here and went red because the ambiguity had resolved.
        let unattributed_not_dropped = moved.iter().filter(|m| m.worker.is_empty()).count() == 1
            && moved.iter().any(|m| m.worker.is_empty() && m.sha == w3);

        let _ = std::fs::remove_dir_all(&dir);
        Ok((kept && no_w4 && unattributed_not_dropped, ordinary))
    })();
    let (red, green) = res.unwrap_or_else(blocked);

    Probe {
        name: "batch: a run records the shas its batch merged, keeping one whose branch has moved off it, unattributed rather than dropped",
        red_fires: red,
        green_passes: green,
    }
}

/// air-i6fd (alerts, 2026-09-06): the landable answer is the same from a worktree as from the
/// main checkout, because main's tip comes from the ref rather than from the running cwd.
///
/// `git::head(repo)` supplied main's tip in three places, and `repo` is whatever directory the
/// command ran in — `main.rs` passes `cli.repo` or the cwd, unnormalised. From a worktree it is
/// that worktree's own head, so the running worker's branch was compared against ITSELF
/// (`is_ancestor(head, head)` is always true), took `select`'s already-in-main path, and left
/// through `Ok(false) => continue`, the one exit here that says nothing. It appeared in neither
/// `landable` nor `skipped` — exactly what `select`'s own doc promises cannot happen (air-6u5,
/// "nothing here is silent"). alerts read `landable: []` from its own worktree and concluded it
/// had nothing to land, in the same snapshot where the batch-ready path — wrong the same way,
/// surfacing differently — called that branch landable on its own.
///
/// Driven through `select` with the worktree as `repo`, which is precisely the value the binary
/// passes when run there. Not through `air status --json`, and that is a finding rather than a
/// shortcut: the snapshot carries `landable` only, so `skipped` and `errors` — the two things
/// air-6u5 added so that a non-qualifying branch says WHY — never reach the JSON at all.
/// Captured separately; this probe reads them where they exist.
///
/// Red: from inside the worktree its own branch is named, and both directories give the same
/// answer. Green: the answer is CORRECT and not merely non-empty — a branch with no green is
/// still skipped naming that precondition, so visibility was not bought by lowering the bar.
fn probe_landable_does_not_depend_on_which_worktree_asked() -> Probe {
    use crate::cmd::status::select;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let git = |at: &std::path::Path, args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(at)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "git {}: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        git(&dir, &["init", "-q", "-b", "main"])?;
        git(&dir, &["commit", "-q", "--allow-empty", "-m", "base"])?;

        // Two workers: `w` green and landable, `nog` with no green at all.
        let wt = dir.join(".claude").join("worktrees").join("w");
        let nog = dir.join(".claude").join("worktrees").join("nog");
        for (name, at) in [("w", &wt), ("nog", &nog)] {
            git(
                &dir,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    name,
                    &at.display().to_string(),
                ],
            )?;
            git(
                at,
                &[
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    &format!("work\n\nBead: zz-{name}"),
                ],
            )?;
        }
        let w_head = git(&wt, &["rev-parse", "HEAD"])?;
        let main_sha = git(&dir, &["rev-parse", "main"])?;

        let l = Ledger::open_for_repo(&dir).map_err(|e| e.to_string())?;
        l.record_claim("zz-w", "w", &[], "t0")
            .map_err(|e| e.to_string())?;
        l.record_claim("zz-nog", "nog", &[], "t0")
            .map_err(|e| e.to_string())?;
        l.record_verify(&VerifyRun {
            id: new_id(),
            worker: "w".into(),
            sha: w_head,
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
            main_sha: Some(main_sha),
        })
        .map_err(|e| e.to_string())?;
        drop(l);

        // The same question, asked from the worktree and from the main checkout.
        let from_wt = select(&wt);
        let from_main = select(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        let workers = |s: &crate::cmd::status::Selection| -> Vec<String> {
            s.landings.iter().map(|l| l.worker.clone()).collect()
        };
        let skipped_for = |s: &crate::cmd::status::Selection, who: &str| -> Vec<&'static str> {
            s.skipped
                .iter()
                .filter(|k| k.worker == who)
                .map(|k| k.check)
                .collect()
        };

        // RED: asked from `w`'s own worktree, `w` is named, and the two directories agree.
        // `w` used to be compared against itself and vanish from both lists.
        let named = workers(&from_wt).iter().any(|x| x == "w");
        let agrees = workers(&from_wt) == workers(&from_main)
            && skipped_for(&from_wt, "nog") == skipped_for(&from_main, "nog");
        let red = named && agrees;

        // GREEN: right, not merely non-empty. The branch with no green is still skipped naming
        // that precondition, it is not landable, and nothing failed — an error here would mean
        // the list was short for a reason that has nothing to do with qualification.
        let green = skipped_for(&from_wt, "nog") == vec!["green-at-head"]
            && !workers(&from_wt).iter().any(|x| x == "nog")
            && from_wt.errors.is_empty()
            && from_main.errors.is_empty();

        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);

    Probe {
        name: "status: landable is the same from a worktree as from the main checkout, and the running worker's own branch is never silently dropped",
        red_fires: red,
        green_passes: green,
    }
}

/// air-hgi9 (an adopter, 2026-09-06, whose w3 met three of these in one night): the refusal
/// says WHICH not-green state it is.
///
/// Four states rendered one sentence, `no green verify recorded at HEAD <sha>`: no green
/// recorded to check, none containing the main it was recorded over, none touching the bead's
/// commits, and the bead having no commit here at all. Only the batch-predates case named what
/// it found. Two of the four have opposite correct responses — wait for the next batch, versus
/// stop waiting for this one and ride the one after — and a worker could not tell which it was
/// looking at.
///
/// The facts were all present and discarded: [`cover`] already walks the candidates, already
/// filters on `contains_main`, and already knows how many commits the bead has. `Scanned`
/// keeps the counts from that same loop, so this adds no git call and no ledger read.
///
/// Red: the four states render four distinct sentences, each naming its own distinguishing
/// number, and the two whose responses differ carry different fixes. Green: the states that
/// were already distinguished still are (a covering green passes, a partial one still names the
/// commit it lacks), and `with_main` counts against the main each run was RECORDED over rather
/// than current main — the air-9ij distinction, which a reader of the field name would not
/// assume and which the sentence therefore says out loud.
fn probe_the_refusal_says_which_not_green_state_it_is() -> Probe {
    use crate::cmd::batch::{Candidate, Scanned, cover, fix_absent, why_absent};

    let commits = |n: usize| -> Vec<crate::cmd::batch::BeadCommit> {
        (0..n)
            .map(|i| crate::cmd::batch::BeadCommit {
                sha: format!("c{i}0000000"),
                subject: format!("work {i}"),
            })
            .collect()
    };
    let cand = |sha: &str, main: bool, contains: &[bool]| Candidate {
        sha: sha.into(),
        worker: "lane".into(),
        contains_main: main,
        contains: contains.to_vec(),
    };

    // The four states, as the scan sees them.
    let none_recorded = cover(&[], &commits(2)).scanned;
    let no_main = cover(&[cand("a", false, &[false, false])], &commits(2)).scanned;
    let untouched = cover(&[cand("b", true, &[false, false])], &commits(2)).scanned;
    let no_commits = cover(&[cand("c", true, &[])], &[]).scanned;

    let says = |s: &Scanned| why_absent("air-hgi9", s);
    let four = [
        says(&none_recorded),
        says(&no_main),
        says(&untouched),
        says(&no_commits),
    ];
    // Distinct sentences, each naming the number that distinguishes it.
    let distinct = four.iter().collect::<std::collections::BTreeSet<_>>().len() == 4
        && four[0].contains("no green verify is recorded to check")
        && four[0].contains(&format!(
            "last {} verify runs",
            crate::cmd::batch::CANDIDATES
        ))
        && four[1].contains("1 recorded green(s) checked")
        && four[1].contains("the main it was recorded over")
        && four[2].contains("1 green(s) contain main")
        && four[2].contains("none contains any of the 2 commit(s)")
        && four[3].contains("`Bead: air-hgi9` trailer");
    // The half that actually differs: wait-for-the-next-batch versus stop-waiting are opposite
    // instructions, and they used to share a fix line.
    let fixes = [
        fix_absent(&none_recorded),
        fix_absent(&no_main),
        fix_absent(&untouched),
        fix_absent(&no_commits),
    ];
    let acts = fixes
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == 4
        && fixes[2].contains("the next batch rather than this one")
        // The no-commits state keeps the green as its SUBJECT: the missing trailer is a fact in
        // the detail. Making the trailer the fix told a worker to add one when their next step
        // was a verify, and an existing probe caught it by going red on this very line.
        && fixes[3].contains("a green at this head")
        && four[3].contains("trailer");
    // No fix names a command the worker must not run under a lane (air-155w).
    let flow_safe = fixes
        .iter()
        .all(|f| !f.contains("air record verify") && !f.contains("git merge main"));
    // And the sentence reaches the worker: the gate renders the reason and the matching fix
    // on the one refusal, rather than the generic line the four used to share.
    let mut f = base_facts();
    f.green_at_head = false;
    f.runs_at_head = (0, 0);
    f.batch_absent = Some(four[2].clone());
    f.batch_absent_fix = Some(fixes[2].to_string());
    let rendered = handover_verdict(&f).missing.iter().any(|m| {
        m.check == "verify-green-at-head"
            && m.detail.contains("no green verify recorded at HEAD")
            && m.detail.contains("none contains any of the 2 commit(s)")
            && m.fix.contains("the next batch rather than this one")
    });
    let red = distinct && acts && flow_safe && rendered;

    // The states that were ALREADY distinguished must stay so, and the counts must not have
    // changed what `cover` decides.
    let covering = cover(&[cand("d", true, &[true, true])], &commits(2));
    let partial = cover(&[cand("e", true, &[true, false])], &commits(2));
    // air-9ij: `with_main` is counted from the filter that asks about the run's RECORDED main,
    // so a candidate disqualified there is not counted as containing main — which is why the
    // sentence says "the main it was recorded over" rather than "main".
    let recorded_main = no_main.candidates == 1 && no_main.with_main == 0;
    let green = covering.covering == Some(("d".to_string(), "lane".to_string()))
        && covering.predates.is_none()
        && partial
            .predates
            .as_ref()
            .is_some_and(|(sha, _, missing, _)| sha == "e" && missing == "c10000000")
        && recorded_main
        && untouched.with_main == 1
        && untouched.touching == 0;

    Probe {
        name: "gate: each not-green state names the fact that distinguishes it, and the two with opposite responses carry different fixes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-et0o (an adopter's worker, 2026-09-06): the peer warning says which tense it is in.
///
/// `is also being edited by w3` was printed identically for a live concurrent edit and for a
/// journal entry left by a worker that had not existed for a fortnight. Their worker spent a
/// stop and four fields of `air holdings` output establishing that nobody was in the file. The
/// cost is not the false alarm: a reader who learns the warning is usually stale stops reading
/// it, and the one time it is live it looks the same.
///
/// The age is read from a column the query already touched, and `air holdings` has printed the
/// tense per holder since air-v7o — this is one surface not saying what its neighbour says.
/// **Not** clean-or-dirty (a `git status` on a hook path whose git budget fails OPEN) and
/// **not** suppression of peers with no live session (it fails toward silence on the case the
/// warning exists for); both reasons are on [`crate::cmd::hook::peer_ages`].
///
/// Red: the sentence a worker actually gets, from a real hook against a real ledger, dates the
/// entry — and the renderer spells a fortnight-old entry and a minutes-old one differently, so
/// the two states a reader has to tell apart are told apart.
///
/// Green: the two things this must not have broken. A genuinely concurrent edit still warns —
/// this bead made no peer silent — and a second edit in the same session stays quiet even
/// after the peer's timestamp moves, because the change-only fingerprint is built from NAMES
/// only. Folding the age into it would change it on every peer edit and turn one warning per
/// session into one per edit.
fn probe_the_peer_warning_dates_the_entry() -> Probe {
    use crate::cmd::hook::peer_ages;

    let at = "2026-09-06T12:00:00Z";
    let stale = [("w3".to_string(), "2026-08-23T12:00:00Z".to_string())];
    let live = [("w2".to_string(), "2026-09-06T11:57:00Z".to_string())];
    let both = [live[0].clone(), stale[0].clone()];
    let renders = peer_ages(&stale, at) == "w3 (14 d ago)"
        && peer_ages(&live, at) == "w2 (3 min ago)"
        && peer_ages(&both, at) == "w2 (3 min ago), w3 (14 d ago)";

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
                return Err(format!(
                    "git {}: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "base"])?;
        std::fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let edit = |session: &str, file: &str| -> Result<String, String> {
            let input = serde_json::json!({
                "hook_event_name": "PreToolUse",
                "session_id": session,
                "cwd": dir.display().to_string(),
                "tool_name": "Edit",
                "tool_input": {"file_path": dir.join("src").join(file).display().to_string()},
            });
            let mut child = air_command(&exe, &dir)
                .arg("hook")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            {
                use std::io::Write;
                let mut stdin = child.stdin.take().ok_or("no stdin")?;
                stdin
                    .write_all(input.to_string().as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        };
        let touch = |worker: &str, file: &str, when: &str| -> Result<(), String> {
            let l = Ledger::open_for_repo(&dir).map_err(|e| e.to_string())?;
            air_hooks::journal::touch(&l, worker, &format!("src/{file}"), Some("s"), when)
                .map_err(|e| e.to_string())
        };

        // A worker that stopped existing years ago is still journaled on the file. Dated far
        // back on purpose: the assertion is "reads in days", and a fixed recent date would
        // start passing for a different reason as the real clock moved.
        touch("w3", "a.rs", "2020-01-01T00:00:00Z")?;
        let said = edit("s1", "a.rs")?;
        let dated = said.contains("is also journaled by w3 (")
            && said.contains(" d ago)")
            && !said.contains("is also being edited by");

        // A peer in the file right now is still warned about. Deliberately NOT asserting the
        // age here: that is the red half's claim, and restating it in the green half would
        // make both halves fail together under the declared mutation — which reports as a
        // mutation that took out the whole guard rather than one that reached a branch.
        touch("w2", "b.rs", &crate::cmd::now())?;
        let live_said = edit("s2", "b.rs")?;
        let still_warns = live_said.contains("is also journaled by w2");

        // The same session edits the same file again, and in between the peer touches it
        // again. Same names, so the warning is not repeated: once per session, not per edit.
        touch("w2", "b.rs", &crate::cmd::now())?;
        let again = edit("s2", "b.rs")?;
        let quiet = !again.contains("journaled by");

        let _ = std::fs::remove_dir_all(&dir);
        Ok((dated, still_warns && quiet))
    })();
    let (dated, kept) = res.unwrap_or_else(blocked);

    Probe {
        name: "hook: the peer warning dates each holder's journal entry, so a fortnight-old one does not read like a live edit",
        red_fires: dated && renders,
        green_passes: kept,
    }
}

/// air-avj (an adopter's coordinator, 2026-09-06, after it cost three workers in one round).
/// The Stop hook fires when a worker is choosing what to do next, and it arrives with the
/// authority of tooling. Under a verify lane its text told them to do the two things a lane
/// exists to prevent: `git merge main`, which moves the head off the sha the lane cut its batch
/// at, and `air record verify`, which is the lane's job. w1 obeyed and lost its membership, w2
/// ignored it at a round trip's cost, w3 asked instead of obeying.
///
/// Red (declared mutation: `flow_dependent` is always false): the Stop hook prints both repairs
/// again, which is the defect exactly. Green: the FACTS are unchanged — every check, every
/// detail, still there, because a worker still has to know why it will be refused; a fix that
/// does not depend on the flow (the `Bead:` trailer) is still printed in full; the pointer to
/// `air handover` appears only when something flow-dependent was dropped; and `air handover`'s
/// own message keeps both repairs, since the CLI is the surface that reads the repo's flow.
fn probe_stop_never_advises_a_lane_worker_to_merge_or_verify() -> Probe {
    use air_hooks::{handover_verdict, stop_message};

    // A worker behind main with no green: both flow-dependent checks fire at once, which is
    // the state the adopter's three workers were in.
    let mut f = base_facts();
    f.green_at_head = false;
    f.main_is_ancestor = false;
    f.main_sha = "aaaaaaaa".into();
    let v = handover_verdict(&f);
    let stop = stop_message(&v, &f.worker, &f.head);

    // RED: neither repair reaches a worker at Stop.
    let red = !stop.contains("git merge main")
        && !stop.contains("air record verify")
        && v.missing.iter().filter(|m| m.flow_dependent).count() == 2;

    // The facts survive: a worker still has to know why it will be refused.
    let facts_kept = v
        .missing
        .iter()
        .all(|m| stop.contains(m.check) && stop.contains(&m.detail));
    let points_at_the_command = stop.contains("air handover");
    // air-155w: this asserted the CLI keeps printing `air record verify`, which was
    // air-avj's design — Stop drops the flow-dependent fix, `air handover` keeps it. That left
    // the refusal itself handing an adopter's lane worker the clause its flow forbids, so the
    // half that survives is "the CLI prints every fix IN FULL", which is what distinguishes it
    // from Stop, and the half that goes is the forbidden command being in the fix at all.
    let cli_prints_every_fix_in_full = v.missing.iter().all(|m| v.message.contains(&m.fix))
        && v.message.contains("git merge main")
        && !v.message.contains("air record verify");

    // A refusal with NOTHING flow-dependent keeps its fix in full and needs no pointer: the
    // trailer is the trailer whatever the flow is.
    let mut g = base_facts();
    g.bead_claimed_or_carried = false;
    g.bead = Some("fd-1".into());
    let cv = handover_verdict(&g);
    let cs = stop_message(&cv, &g.worker, &g.head);
    let flow_free_fix_kept = cv
        .missing
        .iter()
        .filter(|m| !m.flow_dependent)
        .all(|m| cs.contains(&m.fix))
        && !cs.contains("air handover");

    // A pass says what it always said.
    let mut ok = base_facts();
    ok.green_at_head = true;
    let pv = handover_verdict(&ok);
    let pass_unchanged = stop_message(&pv, &ok.worker, &ok.head) == pv.message;

    let green = facts_kept
        && points_at_the_command
        && cli_prints_every_fix_in_full
        && flow_free_fix_kept
        && pass_unchanged;
    Probe {
        name: "hook: the Stop advisory never tells a worker to merge main or record a verify, and names `air handover` instead; a flow-free fix is still printed in full",
        red_fires: red,
        green_passes: green,
    }
}

/// air-cyf: `red_batch_standing` read `latest_runs(Kind::Verify, 20)` and picked the red batch
/// out of that window. Past 20 further verify runs a standing red batch stopped being reported,
/// with nothing said — **and a report that was dropped looked exactly like one that was fixed.**
/// The 20 had no test and no reason recorded beside it, and it failed toward permitting in the
/// one place the fleet is told that nothing may land.
///
/// A round here records dozens of verify runs in an evening (`air audit` counts them), so this
/// is not a theoretical horizon.
///
/// Red: a red batch with 25 later verify runs on top of it is STILL reported. That is the exact
/// case the window dropped, and the number is one more than the window that used to exist plus
/// margin, so a smaller window than 25 cannot pass it either.
///
/// Green: the only reason a standing red stops being reported is that it was fixed — a later
/// green that carries EVERY member supersedes it, and one that carries only some does not. And
/// two runs that are not batches are not reported as one: a red at a worker's own head carries
/// no members, and a killed run is no verdict at all rather than a red.
fn probe_a_standing_red_batch_is_not_aged_out_by_later_runs() -> Probe {
    use crate::cmd::batch::{RedBatch, red_batch_standing, superseded_by};
    use air_ledger::landings::Member;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let l = Ledger::open_in(&dir).map_err(|e| e.to_string())?;
        let mut at: u32 = 0;
        let mut record = |sha: &str, exit: i32, members: Vec<Member>| -> Result<String, String> {
            at = at.saturating_add(1);
            let t = format!("2026-09-06T00:{at:02}:00Z");
            l.record_verify(&VerifyRun {
                id: new_id(),
                worker: "lane".into(),
                sha: sha.into(),
                kind: Kind::Verify,
                exit_code: exit,
                trigger: "selftest".into(),
                failing_step: None,
                started_at: t.clone(),
                finished_at: t.clone(),
                log_path: None,
                command: None,
                duration_ms: None,
                output_bytes: None,
                dirty: false,
                tree: None,
                members,
                main_sha: None,
            })
            .map_err(|e| e.to_string())?;
            Ok(t)
        };
        let m = |w: &str, sha: &str| Member {
            worker: w.into(),
            sha: sha.into(),
        };
        let members = vec![m("alpha", "a1a1a1a1a1"), m("beta", "b2b2b2b2b2")];

        // The batch goes red, then the fleet keeps working: 25 ordinary runs on top of it,
        // five past the window that used to exist.
        let red_at = record("batch1234", 2, members.clone())?;
        for i in 0..25 {
            record(&format!("worker{i:04}"), i32::from(i % 3 == 0), Vec::new())?;
        }
        // `repo` is a directory with no git in it, so `is_ancestor` answers false for every
        // pair: no green here carries anything, which isolates the AGE question from the
        // supersession one.
        let still_there = red_batch_standing(&l, &dir).is_some_and(|b| b.sha == "batch1234");

        // Not a batch: a red at a worker's own head carries no members, and a killed run is no
        // verdict. Neither may be reported as a standing red batch.
        let dir2 = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir2).map_err(|e| e.to_string())?;
        let l2 = Ledger::open_in(&dir2).map_err(|e| e.to_string())?;
        l2.record_verify(&VerifyRun {
            id: new_id(),
            worker: "w".into(),
            sha: "plainred1".into(),
            kind: Kind::Verify,
            exit_code: 2,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: "2026-09-06T01:00:00Z".into(),
            finished_at: "2026-09-06T01:00:00Z".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: None,
            members: Vec::new(),
            main_sha: None,
        })
        .map_err(|e| e.to_string())?;
        l2.record_verify(&VerifyRun {
            id: new_id(),
            worker: "lane".into(),
            sha: "killedbatch".into(),
            kind: Kind::Verify,
            // 137, not any non-zero: `KILLED_EXITS` is [137, 143] and a probe that used 130
            // would be asserting that an ordinary red batch is not a batch.
            exit_code: 137,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: "2026-09-06T02:00:00Z".into(),
            finished_at: "2026-09-06T02:00:00Z".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: None,
            members: members.clone(),
            main_sha: None,
        })
        .map_err(|e| e.to_string())?;
        let neither_is_a_batch = red_batch_standing(&l2, &dir2).is_none();

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
        let _ = red_at;
        Ok((still_there, neither_is_a_batch))
    })();
    let (still_there, neither_is_a_batch) = res.unwrap_or((false, false));

    // Supersession, pure: only a green carrying EVERY member fixes the batch. Driven through
    // an ancestry oracle rather than a repo, so the rule is visible instead of inferred.
    let red = RedBatch {
        sha: "batch1234".into(),
        worker: "lane".into(),
        at: "2026-09-06T00:01:00Z".into(),
        members: vec![
            Member {
                worker: "alpha".into(),
                sha: "a1".into(),
            },
            Member {
                worker: "beta".into(),
                sha: "b2".into(),
            },
        ],
        log_path: None,
    };
    let green = |sha: &str, at: &str| VerifyRun {
        id: new_id(),
        worker: "lane".into(),
        sha: sha.into(),
        kind: Kind::Verify,
        exit_code: 0,
        trigger: "selftest".into(),
        failing_step: None,
        started_at: at.into(),
        finished_at: at.into(),
        log_path: None,
        command: None,
        duration_ms: None,
        output_bytes: None,
        dirty: false,
        tree: None,
        members: Vec::new(),
        main_sha: None,
    };
    // An empty directory: `is_ancestor` cannot answer, so nothing carries anything.
    let nowhere = Path::new("/nonexistent-air-selftest");
    let carries_nothing = !superseded_by(nowhere, &red, &[green("g1", "2026-09-06T09:00:00Z")]);
    // A green BEFORE the batch never supersedes it, whatever it carries.
    let earlier_never = !superseded_by(nowhere, &red, &[green("g0", "2026-09-06T00:00:00Z")]);

    Probe {
        name: "batch: a standing red batch is reported until a green carries every member, and is never aged out by later runs",
        red_fires: still_there,
        green_passes: neither_is_a_batch && carries_nothing && earlier_never,
    }
}

/// air-x1ha (verify's capture, 2026-09-06, reproduced by hand): a worker typed `air-ahl`, bd
/// resolved and claimed `air-ahlf`, Air wrote its row under the typed PREFIX, and the next
/// status reconcile asked bd about the prefix, got nothing, and released the claim while the
/// work continued. The coordinator saw a worker that had abandoned a bead it was still
/// building; the worker saw nothing at all. Two stores holding different ids for one bead is
/// what air-uir prevents a layer up.
///
/// The fake bd here is bd 1.2.2's real shape in the two ways that matter: `show` resolves an
/// unambiguous prefix and answers with the CANONICAL id (checked against the real bd,
/// 2026-09-06: `bd show zz-bd --json` → `"id": "zz-bdz"`), and it OMITS an id it does not know
/// while still exiting 0.
///
/// Red (declared mutation: the reconcile releases whenever bd did not positively hold the
/// bead): a claim under an id bd cannot resolve is released again, which is the defect. Green:
/// `air claim` on a prefix writes the row under the id bd resolved, so a prefix claim and a
/// full-id claim are the same row; a claim bd KNOWS and no longer holds is still released,
/// because that is what the reconcile is for; and the kept row is reported with the id it
/// looked up, not just a count.
fn probe_a_prefix_claim_is_recorded_and_survives_the_reconcile() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dir = dir.canonicalize().map_err(|e| e.to_string())?;
        let g = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["init", "-q", "-b", "main"])
            .output()
            .map_err(|e| e.to_string())?;
        if !g.status.success() {
            return Err(String::from_utf8_lossy(&g.stderr).to_string());
        }
        let script = dir.join("bd");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nd='{d}'\necho \"$@\" >> \"$d/bd.log\"\ncase \"$1\" in\n  \
                 --version) echo 'bd version 1.2.2'; exit 0;;\n  \
                 show) shift; out=''\n    for id in \"$@\"; do case \"$id\" in --*) continue;; esac\n      \
                 case \"$id\" in\n        \
                 zz-pre|zz-full) row='{{\"id\":\"zz-full\",\"title\":\"t\",\"status\":\"open\",\"labels\":[],\"issue_type\":\"task\"}}';;\n        \
                 zz-gone) row='{{\"id\":\"zz-gone\",\"title\":\"t\",\"status\":\"open\",\"labels\":[],\"issue_type\":\"task\"}}';;\n        \
                 *) row='';;\n      \
                 esac\n      \
                 [ -n \"$row\" ] && out=\"$out${{out:+,}}$row\"\n    done\n    \
                 printf '%s\\n' \"[$out]\"; exit 0;;\n  \
                 list) echo '[]'; exit 0;;\n  \
                 ready) echo '[]'; exit 0;;\n  \
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
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let air = |args: &[&str]| -> Result<String, String> {
            let out = air_command(&exe, &dir)
                .env("AIR_BD_BIN", &script)
                .args(args)
                .output()
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        };
        let rows = || -> Vec<(String, Option<String>)> {
            rusqlite::Connection::open(dir.join(".air").join("ledger.db"))
                .and_then(|c| {
                    let mut st =
                        c.prepare("SELECT bead, release_reason FROM claims ORDER BY bead")?;
                    let v = st
                        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    Ok(v)
                })
                .unwrap_or_default()
        };

        // A worker types the PREFIX. bd resolves it to zz-full.
        air(&["claim", "zz-pre"])?;
        let claimed_canonical = rows().iter().any(|(b, _)| b == "zz-full")
            && !rows().iter().any(|(b, _)| b == "zz-pre");

        // A row bd KNOWS and no longer holds in progress, and a row under an id bd cannot
        // resolve at all — the shape a pre-air-x1ha prefix claim leaves behind.
        {
            let l = Ledger::open_for_repo(&dir).map_err(|e| e.to_string())?;
            l.record_claim("zz-gone", "probe", &[], "t0")
                .map_err(|e| e.to_string())?;
            l.record_claim("zz-nope", "probe", &[], "t0")
                .map_err(|e| e.to_string())?;
        }
        let status = air(&["status"])?;
        let after = rows();
        let held = |b: &str| after.iter().any(|(x, r)| x == b && r.is_none());
        let released = |b: &str| after.iter().any(|(x, r)| x == b && r.is_some());

        // RED: the unresolvable row survives. That is the whole bug.
        let red = held("zz-nope");
        let green = claimed_canonical
            // The reconcile still does its job for a bead bd knows and no longer holds.
            && released("zz-gone")
            // The kept row is NAMED, with what was looked up, not just counted.
            && status.contains("zz-nope")
            && status.contains("could not resolve");
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "claim: a prefix claim is recorded under the id bd resolved and survives the reconcile; a bead bd no longer holds is still released",
        red_fires: red,
        green_passes: green,
    }
}

/// air-3jv5: a demonstration produced a worker a reader could not tell from a live one.
///
/// A synthetic Stop event was piped into `air hook` against the REAL ledger to show what the
/// hook says. Air wrote a `sessions` row for it and the channel pushed `session_joined` to the
/// whole fleet — for a session that did not exist. Nothing broke and the real session was
/// untouched, but `sessions` is the table `air status`, every attention condition and the
/// stopped-session line all read, so the cost was only luck.
///
/// **The fix reports rather than refuses, and the direction is the whole argument.** Refusing
/// to write a row without a transcript would lose a REAL worker from `air status` the day a
/// harness omitted the field, and losing a live worker is worse than showing a synthetic one.
/// The id's shape was the other candidate and is worse still: it is a guess about a format
/// Air does not own, and it fails the same wrong way.
///
/// Red: a row with no transcript is named `NO TRANSCRIPT` on its `air status` line, and the
/// channel does not announce it as a worker joining.
///
/// Green: three things that keep this from being a filter that hides real workers. A row WITH
/// a transcript is announced exactly as before. A synthetic row is still IN the snapshot,
/// still rendered, still carrying its worker and state — reported, not suppressed. And a
/// LEAVE is announced whatever the row looked like, because the id was in the set and is gone,
/// and staying silent about a real departure costs the coordinator the thing the condition is
/// for.
fn probe_a_row_with_no_transcript_is_named_and_never_announced() -> Probe {
    use crate::cmd::mcp::session_changes;
    use crate::cmd::status::{Session, Snapshot, WorkerView, render_for_probe};

    let sess = |id: &str, has_transcript: bool| Session {
        session_id: id.into(),
        state: "idle".into(),
        detail: None,
        changed_at: "2026-09-06T11:16:35Z".into(),
        pid: Some(97487),
        pid_alive: Some(true),
        project: "air".into(),
        model: String::new(),
        enforce: None,
        has_transcript,
        stopped: None,
    };
    let row = |id: &str, has_transcript: bool| {
        (
            "alerts".to_string(),
            "worker".to_string(),
            sess(id, has_transcript),
        )
    };

    // The demonstration, with the real worker already known: a second row appears for the same
    // worker, from no session.
    let mut known = Some(
        ["a23513fb".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<String>>(),
    );
    let synthetic_join = session_changes(
        &mut known,
        &[row("a23513fb", true), row("air-avj-demo", false)],
    );
    // Rendered: the row is present AND named.
    let shown = render_for_probe(&Snapshot {
        workers: vec![WorkerView {
            worker: "alerts".into(),
            role: "worker".into(),
            session: Some(sess("air-avj-demo", false)),
            ..Default::default()
        }],
        ..Default::default()
    });
    // And the fleet-wide count, over EVERY row rather than the rendered ones: the per-worker
    // view keeps only the latest row per worker, so a synthetic row goes invisible there the
    // moment the real session emits a hook — which is how the one that pushed `session_joined`
    // sat in the table unseen.
    let counted = render_for_probe(&Snapshot {
        sessions: vec![row("a23513fb", true), row("air-avj-demo", false)],
        ..Default::default()
    });
    let red = synthetic_join.is_empty()
        && shown.contains("NO TRANSCRIPT")
        && shown.contains("alerts")
        && counted.contains("1 of 2 row(s) have no transcript");

    // A real session joining is announced, exactly as before.
    let mut known2 = Some(
        ["a23513fb".to_string()]
            .into_iter()
            .collect::<std::collections::BTreeSet<String>>(),
    );
    let real_join = session_changes(&mut known2, &[row("a23513fb", true), row("bbbbbbbb", true)]);
    let announced = real_join.len() == 1
        && real_join
            .first()
            .is_some_and(|(k, w, _)| *k == "session_joined" && w == "alerts");

    // The synthetic id was SEEDED, so it is not announced on a later tick either; and when it
    // vanishes, the leave IS announced, because a missing id is a fact whatever wrote it.
    let never_late = session_changes(&mut known, &[row("air-avj-demo", false)])
        .iter()
        .all(|(k, _, _)| *k != "session_joined");
    let leave = session_changes(&mut known, &[])
        .iter()
        .any(|(k, _, _)| *k == "session_left");
    // Reported, not suppressed: a snapshot with the synthetic row still has it.
    // Silence means checked-and-none, not not-looked: with every row backed by a transcript
    // the line is absent, and the count it would have printed is the whole set.
    let quiet = !render_for_probe(&Snapshot {
        sessions: vec![row("a23513fb", true)],
        ..Default::default()
    })
    .contains("no transcript behind them");
    let still_rendered = render_for_probe(&Snapshot {
        workers: vec![WorkerView {
            worker: "alerts".into(),
            role: "worker".into(),
            session: Some(sess("air-avj-demo", false)),
            ..Default::default()
        }],
        ..Default::default()
    })
    .contains("idle since");

    Probe {
        name: "sessions: a row no session is behind is named in status and never announced as a worker joining, and a real one still is",
        red_fires: red,
        green_passes: announced && never_late && leave && still_rendered && quiet,
    }
}

/// air-e21v: `air selftest --json` emitted a stray `out` before the array, so it failed to
/// parse at line 1 column 1 — and `--prove` parses exactly that stream, so **every declared
/// mutation reported BROKEN and the suite's mutation evidence was dead** for as long as it took
/// anyone to run a 30-minute command. Ordinary `air selftest` was unaffected and all probes
/// passed, which is why nothing noticed: the gate worked, only the evidence behind it did not.
///
/// This probe is the CONTRACT half, and it is pure. The end-to-end half — spawn
/// `air selftest --json` and parse it — was written, measured and NOT kept: it costs a second
/// full suite, 51 s on a quiet machine and 117 s under a round's load, against a 17 s suite and
/// a 56-85 s `make verify`. "Tests are optimized for speed, always. Per-test cost is a
/// first-class constraint" (CLAUDE.md) settles that. What replaces it costs nothing: `prove`
/// checks the UNMUTATED child parses before it applies a single mutation, so the 82 runs that
/// would each have reported BROKEN cannot happen, and the one that fails names the cause.
///
/// Red: the shape `--prove` requires is accepted, and a stream with anything before the array
/// is refused — `serde_json` refuses `out\n[…]` at line 1, which is precisely what happened.
/// Green: an empty array is not evidence of a suite either, and `baseline_defect` says what is
/// wrong rather than only that something is.
fn probe_selftest_json_is_only_the_array() -> Probe {
    let good = r#"[{"name":"n","red_fires":true,"green_passes":true}]"#;
    let polluted = "out\n[{\"name\":\"n\",\"red_fires\":true,\"green_passes\":true}]";
    Probe {
        name: "selftest: --prove refuses a polluted baseline instead of calling every mutation broken",
        red_fires: baseline_defect(polluted)
            .is_some_and(|d| d.contains("before the array") || d.contains("column 1"))
            && baseline_defect(good).is_none(),
        green_passes: baseline_defect("[]").is_some_and(|d| d.contains("no probes"))
            && baseline_defect("").is_some(),
    }
}

/// What is wrong with a child's `--json` stream, or `None` when it is what `--prove` needs
/// (air-e21v). Pure, so the check costs nothing and the message is testable.
fn baseline_defect(text: &str) -> Option<String> {
    match serde_json::from_str::<Vec<ProbeOut>>(text) {
        Ok(v) if v.is_empty() => Some("no probes in the array".to_string()),
        Ok(_) => None,
        Err(e) => {
            let head: String = text.trim_start().chars().take(40).collect();
            Some(format!(
                "{e}; something is written before the array. `air selftest --json` must emit \
                 ONLY the array, and a probe that writes to this process's stdout breaks it \
                 (air-e21v: a probe passed `run_tee` the real stdout). Stream begins: {head:?}"
            ))
        }
    }
}

/// air-dwq5 (an adopter's w2 via verify, 2026-09-06): Air embeds the commit it was built from,
/// writes it into `installed.json`, reads it for the install-lag check — and told nobody.
///
/// One crate version covered a round of behaviour changes, because lanes cut no release rows
/// (air-mir, a deliberate trade). Measured that night: the installed binary and a branch build
/// both printed `air 0.2.19` while emitting different Stop-hook advice, one corrected by
/// air-avj and one not. The cost lands on a reader rather than a lane: someone reconstructing
/// the round sees 0.2.19 everywhere, looks up what 0.2.19 fixed, and concludes three workers
/// ignored advice that had already been corrected. The fix is not to reverse the trade; it is
/// to say the fact Air already has.
///
/// Red (declared mutation: the line drops the build and prints the version alone): the string
/// stops distinguishing two binaries, which is the defect exactly — and it still LOOKS like a
/// version, which is why it survived a round unnoticed.
///
/// Green, against the REAL binary rather than the functions it calls, because the bug was that
/// four surfaces did not reach the fact: `--version` carries the build, `--version --json` is
/// parseable JSON rather than the bare string it used to print, and `doctor --json` and
/// `status --json` carry the same object — the same, not a second copy that can drift.
fn probe_the_build_reaches_a_reader() -> Probe {
    use crate::cmd::install::{BUILD, SURFACE_VERSION, version_json, version_line};

    // RED: the one thing a crate version cannot say.
    let red = version_line().contains(BUILD)
        && !BUILD.is_empty()
        && version_json()
            .get("surface_version")
            .and_then(serde_json::Value::as_u64)
            == Some(u64::from(SURFACE_VERSION));

    let res = (|| -> Result<bool, String> {
        let dir = probe_repo()?;
        let out = (|| -> Result<bool, String> {
            probe_git(&dir, &["init", "-q", "-b", "main"])?;
            probe_git(&dir, &["commit", "-q", "--allow-empty", "-m", "a"])?;
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let run = |args: &[&str]| -> Result<String, String> {
                let o = air_command(&exe, &dir)
                    .args(args)
                    .output()
                    .map_err(|e| e.to_string())?;
                Ok(String::from_utf8_lossy(&o.stdout).to_string())
            };
            let want = version_json();
            // `--version --json` is JSON. It printed the plain string, which is the one shape
            // a JSON reader cannot parse.
            let vj: serde_json::Value = serde_json::from_str(run(&["--version", "--json"])?.trim())
                .map_err(|e| e.to_string())?;
            // The text line names the build a reader has to attribute a log line to.
            let vt = run(&["--version"])?;
            // Both diagnostic surfaces carry the SAME object.
            let doc: serde_json::Value =
                serde_json::from_str(&run(&["--json", "doctor"])?).map_err(|e| e.to_string())?;
            let st: serde_json::Value =
                serde_json::from_str(&run(&["--json", "status"])?).map_err(|e| e.to_string())?;
            Ok(vj == want
                && vt.contains(BUILD)
                && vt.contains(env!("CARGO_PKG_VERSION"))
                && doc.get("air") == Some(&want)
                && st.get("air") == Some(&want))
        })();
        std::fs::remove_dir_all(&dir).ok();
        out
    })();

    Probe {
        name: "version: air says which binary it is — the build reaches --version, --version --json is JSON, and doctor and status carry the same object",
        red_fires: red,
        green_passes: res.unwrap_or(false),
    }
}

/// air-155w: an adopter's w1 was refused with `git merge main && air record verify -- make
/// verify`. Under a verify lane the first clause is right and necessary and the second is the
/// one thing that worker must not do — the lane records greens, and a worker's competes with
/// it.
///
/// **This is the nastiest shape in the family, and the reason is worth keeping.** Obeying the
/// first clause and ignoring the second is exactly correct, so following the line WORKS: the
/// worker gets a good outcome and learns the wrong habit, and nothing ever contradicts it. A
/// fix that failed outright would have been found in one use. Their worker noticed and reported
/// it rather than quietly doing the right half — a norm holding a gap, not a mechanism.
///
/// air-avj marked these checks `flow_dependent` and changed what the STOP HOOK prints, leaving
/// the refusal's own fix string carrying the repair. So the class is not "a message was wrong"
/// but **"a decision was copied to more places than anyone enumerated"**, and one of the two
/// surfaces restating it survived the fix for itself.
///
/// Asserted over VERDICTS rather than over source text, because a source scan cannot see the
/// case alerts found: the digest check's order note carries the same repair inside a check that
/// is correctly `flow_dependent: false`, since writing a digest IS flow-free. The flag marks
/// checks; repairs also live inside checks that are not themselves flow-dependent.
///
/// Red: across the fact-space that produces each flow-dependent check, no fix asserts
/// `air record verify`, and the digest order note does not either.
///
/// Green: the parts that must NOT have changed. `main-merged` still names `git merge main`,
/// which is required under both flows; the diagnosis still says "main is not an ancestor of
/// HEAD" and still dates main's move, which is the half that works and which the adopter counts
/// refusals by; and every refusal still names a command, since one that names none is worse
/// than one that names the wrong one.
fn probe_no_flow_dependent_fix_asserts_a_forbidden_repair() -> Probe {
    use air_hooks::gate::{GateFacts, MainMove, handover_verdict};

    let base = || GateFacts {
        worker: "w1".into(),
        head: "687ebe1aaaa".into(),
        main_sha: "ca4fe04bbbb".into(),
        ..GateFacts::default()
    };
    // Every shape that reaches a flow-dependent fix: no green; a flaky head; a tree green; a
    // batch cut before the last commit; and main having moved under the worker.
    let cases: Vec<GateFacts> = vec![
        base(),
        GateFacts {
            runs_at_head: (1, 1),
            ..base()
        },
        GateFacts {
            tree_green: Some("this exact tree is green at deadbee".into()),
            ..base()
        },
        GateFacts {
            batch_predates: Some("the lane's batch was cut before your last commit".into()),
            ..base()
        },
        GateFacts {
            main_is_ancestor: false,
            main_moved: Some(MainMove {
                merge_commit: "abcdef0999".into(),
                worker: "w4".into(),
                at: "2026-09-06T13:00:00Z".into(),
                ago_secs: Some(1140),
            }),
            ..base()
        },
    ];
    let verdicts: Vec<_> = cases.iter().map(handover_verdict).collect();

    // Not one flow-dependent fix, anywhere in that space, tells a worker to record a verify.
    let none_forbidden = verdicts.iter().all(|v| {
        v.missing
            .iter()
            .filter(|m| m.flow_dependent)
            .all(|m| !m.fix.contains("air record verify"))
    });
    // And neither does the digest check's order note, which is not flow-dependent and carried
    // the same repair anyway.
    let digest = handover_verdict(&GateFacts {
        green_at_head: true,
        digest_dir: Some("docs/digests".into()),
        bead: Some("air-1".into()),
        ..base()
    });
    let note_clean = digest
        .missing
        .iter()
        .all(|m| !m.fix.contains("air record verify -- make verify"));
    let saw_flow_dependent = verdicts
        .iter()
        .any(|v| v.missing.iter().any(|m| m.flow_dependent));

    // The half that had to survive untouched.
    let moved = verdicts.last();
    let diagnosis_intact = moved.is_some_and(|v| {
        v.missing.iter().any(|m| {
            m.check == "main-merged"
                && m.detail.contains("main is not an ancestor of HEAD")
                && m.detail.contains("(landing from w4)")
                && m.fix.contains("git merge main")
        })
    });
    // Every refusal still names something to run.
    let names_a_command = verdicts
        .iter()
        .filter(|v| !v.pass)
        .all(|v| v.missing.iter().all(|m| !m.fix.trim().is_empty()));

    Probe {
        name: "gate: no flow-dependent fix tells a worker to record a verify, and the main-moved diagnosis is unchanged",
        red_fires: saw_flow_dependent && none_forbidden && note_clean,
        green_passes: diagnosis_intact && names_a_command,
    }
}

/// air-vsvt (an adopter's verification lane, 2026-09-06): its batch-red lines named members
/// that were not in the batch and omitted members that were, five times in one night.
///
/// The reporting path was NOT the defect and this probe pins that too: `red_batches_of` copies
/// `VerifyRun.members` off the row and `red_batch_line` renders exactly those, so the line has
/// always read what was recorded. The defect is one step earlier, in what gets recorded.
/// `members_of` asked "is this worktree's head an ancestor of the batch" — a question about
/// where a branch is NOW, answered while recording a fact about what the batch WAS. A worker
/// that commits between the lane's merge and the lane's `air record` stops being an ancestor
/// and drops out, and because the list goes on the row it is then wrong for good.
///
/// Red (declared mutation: the member sha is the worktree's head again instead of the merge
/// base): a branch that moved after the batch took it is recorded at the wrong sha, which is
/// the defect. Green: the recorded sha is the one the batch contains and does not move when the
/// branch does; a branch the batch never took has its fork point in main and is excluded, so
/// this cannot invent a member either; and the rendered line reports exactly the recorded
/// members, unchanged.
fn probe_batch_members_are_the_shas_the_batch_took() -> Probe {
    use crate::cmd::batch::{red_batch_line, red_batches_of};

    let res = (|| -> Result<(bool, bool), String> {
        let dir = probe_repo()?;
        let out = (|| -> Result<(bool, bool), String> {
            let g = |args: &[&str]| probe_git(&dir, args);
            g(&["init", "-q", "-b", "main"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "base"])?;
            // A worker branch, and the sha a batch would take from it.
            g(&["checkout", "-q", "-b", "w"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "work"])?;
            let taken = g(&["rev-parse", "HEAD"])?;
            // The batch merges it.
            g(&["checkout", "-q", "-b", "lane", "main"])?;
            g(&["merge", "-q", "--no-ff", "w", "-m", "batch: w"])?;
            let batch = g(&["rev-parse", "HEAD"])?;
            // The worker commits again: its head is no longer an ancestor of the batch.
            g(&["checkout", "-q", "w"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "after the cut"])?;
            let moved = g(&["rev-parse", "HEAD"])?;
            let still_ancestor =
                probe_git(&dir, &["merge-base", "--is-ancestor", &moved, &batch]).is_ok();
            // A branch the batch never took.
            g(&["checkout", "-q", "-b", "other", "main"])?;
            g(&["commit", "-q", "--allow-empty", "-m", "untaken"])?;
            let untaken = g(&["rev-parse", "HEAD"])?;

            let base_of = |sha: &str| crate::git::merge_base(&dir, sha, &batch);
            let in_main = |sha: &str| crate::git::is_ancestor(&dir, sha, "main").unwrap_or(false);

            // RED: the branch really has moved off the batch, and the sha the batch took is
            // recoverable anyway. Both halves, or the case is not the one that bit the adopter.
            let red = !still_ancestor && base_of(&moved).as_deref() == Some(taken.as_str());

            // The recorded sha does not move when the branch does.
            let stable = base_of(&moved) == base_of(&taken);
            // A branch the batch never took resolves into main and is excluded by the same
            // test as before, so nothing is invented.
            let not_invented = base_of(&untaken).is_some_and(|b| in_main(&b));
            // The taken branch is NOT excluded by that test.
            let kept = base_of(&moved).is_some_and(|b| !in_main(&b));
            // The renderer reports exactly what was recorded — never recomputed.
            let run = air_ledger::verify::VerifyRun {
                id: new_id(),
                worker: "lane".into(),
                sha: batch.clone(),
                kind: Kind::Verify,
                exit_code: 2,
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
                members: vec![air_ledger::landings::Member {
                    worker: "w".into(),
                    sha: taken.clone(),
                }],
                main_sha: None,
            };
            let reported = red_batches_of(std::slice::from_ref(&run));
            let line_is_the_record = reported.first().is_some_and(|b| {
                b.members.len() == 1
                    && b.members.first().is_some_and(|m| m.sha == taken)
                    && red_batch_line(b).contains(taken.get(..8).unwrap_or(&taken))
            });

            Ok((red, stable && not_invented && kept && line_is_the_record))
        })();
        std::fs::remove_dir_all(&dir).ok();
        out
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "batch: a run records the sha the batch TOOK from each branch, which does not move when that branch does, and the red line reports exactly what was recorded",
        red_fires: red,
        green_passes: green,
    }
}

/// air-zqmi: the channel told an adopter's coordinator that w3 had handed over `ad-gjw8x`
/// without a green at HEAD. w3's first and only attempt on that bead SUCCEEDED, as did its
/// other two; there was no refusal and so no refusal text. The coordinator acted on a reported
/// refusal that never happened and a worker spent a message establishing it.
///
/// The event log says it exactly: at 13:51:59 the `PreToolUse` line for that close reads
/// `decision: pass`. `handover_gate` stamped the claim anyway — the stamp ran on every
/// hand-over command the gate saw — while `handover-not-green` reads that counter and says
/// "handed over N time(s) without green verify at HEAD". So a worker whose closes all passed
/// was reported to the whole fleet as having failed.
///
/// **Same class as air-eiv, one layer over.** That was a QUERY counting as an attempt:
/// `air handover`, the documented diagnostic, raised the alarm on the worker who ran it. This
/// is a SUCCESS counting as one. The rule both settle on: only a hand-over that did not go
/// through is an attempt.
///
/// The cost is not the wrong line. Their coordinator told its workers to disregard the
/// condition and stopped acting on it for the round, because under a lane its alerts were
/// evidence that closes were working — and a coordinator who ignores a condition is worse off
/// than one who never had it, if a real refusal ever arrives on the same text.
///
/// Red: a close the gate REFUSES stamps one attempt.
///
/// Green: a close the gate PASSES stamps none and clears what was there. The clearing is the
/// half that is easy to miss: without it the counter is a high-water mark, so one early
/// refusal keeps the condition firing after a clean close, which is the same false positive
/// arriving a few minutes later.
fn probe_only_a_failed_handover_counts_as_an_attempt() -> Probe {
    use crate::cmd::hook::handover_gate;
    use air_hooks::HookOutcome;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            crate::git::run(&dir, args).map_err(|e| e.to_string())
        };
        g(&["init", "-q", "-b", "main", "."])?;
        g(&["config", "user.email", "a@b"])?;
        g(&["config", "user.name", "a"])?;
        std::fs::write(dir.join("f"), "x").map_err(|e| e.to_string())?;
        g(&["add", "-A"])?;
        g(&["commit", "-qm", "seed"])?;
        let head = g(&["rev-parse", "HEAD"])?;

        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("zz-1", "w3", &[], "t0")
            .map_err(|e| e.to_string())?;
        let cmd = "bd close zz-1 --reason 'done'";
        let attempts = || -> i64 {
            l.open_claims()
                .unwrap_or_default()
                .iter()
                .find(|c| c.bead == "zz-1")
                .map_or(-1, |c| c.handover_attempts)
        };

        // No green: the gate refuses, and THAT is an attempt.
        handover_gate(&l, "w3", &dir, cmd, true)?;
        let after_refusal = attempts();

        // Now green at that head, so the same close passes.
        l.record_verify(&VerifyRun {
            id: new_id(),
            worker: "w3".into(),
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
            members: Vec::new(),
            main_sha: None,
        })
        .map_err(|e| e.to_string())?;
        let d = handover_gate(&l, "w3", &dir, cmd, true)?;
        let passed = matches!(d.outcome, HookOutcome::Allow { context: None });
        let after_pass = attempts();

        let _ = std::fs::remove_dir_all(&dir);
        // Red: the refusal counted. Green: the pass counted nothing AND cleared the one before.
        Ok((after_refusal == 1, passed && after_pass == 0))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "handover: only a hand-over the gate refused counts as an attempt, and one that passes clears the count",
        red_fires: red,
        green_passes: green,
    }
}

/// air-rud0 (an adopter, after their batch 22, with the rendering proposed by their lane): the
/// landing's clause report gave a REASON when it could not read a clause and a bare `ok` when
/// it could, so the weaker claim wore the stronger form.
///
/// `ok` means a lookup matched — the merge touched a path the clause names, or a green is
/// recorded at the landed sha. It does not mean anything about the clause's substance was
/// checked. Their batch 22 printed `ok` beside a clause about sections no longer carrying a
/// sequential number, discharged because the merge touched that file; it would have printed the
/// same had the change renamed a variable in it. Their lane's phrase, and it is the bead's:
/// **a lookup wearing the clothes of a judgement.** It is right in this case and right most of
/// the time, which is what made it invisible, and a nine-bead landing is read by scanning this
/// column.
///
/// Red (declared mutation: the discharged line goes back to a bare mark): a reader scanning
/// verdicts sees `ok` with nothing beside it, which is the defect. Green: the lookup is on the
/// SAME line as the tick and names the path that discharged it; the two honest branches are
/// untouched, keeping their reason on the line beneath; and — the half that matters most — the
/// JUDGEMENT is unchanged, so a clause naming a file the merge left alone is still `MISS` and
/// prose is still `?`. Nothing here reads a clause's meaning.
fn probe_a_discharged_clause_names_its_lookup() -> Probe {
    use crate::cmd::acceptance::{Evidence, Verdict, judge, judge_clauses, report};

    let changed = ["docs/rules/roles.md".to_string()];
    let tree = [
        "docs/rules/roles.md".to_string(),
        "docs/untouched.md".to_string(),
    ];
    let ev = Evidence {
        green_at_landed: true,
        changed: &changed,
        tree: &tree,
    };
    let touched = "docs/rules/roles.md names the rule.";
    let left_alone = "docs/untouched.md gains a section.";
    let prose = "The reviewer is happy with it.";

    let r = report(&[judge_clauses(
        "zz-1",
        vec![touched.into(), left_alone.into(), prose.into()],
        &ev,
    )]);

    // RED: the tick carries its lookup, on its own line, where the column is scanned.
    let red = r.contains("ok (the merge changed docs/rules/roles.md) — docs/rules/roles.md")
        && !r.contains("ok   docs/rules/roles.md");

    // The judgement did not move: a lookup, never a reading of the clause.
    let judged_the_same = matches!(judge(touched, &ev), Verdict::Discharged { .. })
        && matches!(judge(left_alone, &ev), Verdict::Unevidenced { .. })
        && matches!(judge(prose, &ev), Verdict::Undecidable { .. });
    // The two honest branches are untouched, reason on the line beneath.
    let honest_branches_untouched = r.contains(&format!("MISS {left_alone}"))
        && r.contains("the merge did not change docs/untouched.md")
        && r.contains(&format!("?    {prose}"));
    // The discharged reason appears once, not twice: the second line went with the move.
    let not_duplicated = r.matches("the merge changed docs/rules/roles.md").count() == 1;
    // A green-backed clause names ITS lookup too, not just a path-backed one.
    let g = report(&[judge_clauses(
        "zz-2",
        vec!["make verify green.".into()],
        &ev,
    )]);
    let green_clause_named = g.contains("ok (a green verify is recorded at the landed sha) —");

    let green =
        judged_the_same && honest_branches_untouched && not_duplicated && green_clause_named;
    Probe {
        name: "land: a discharged acceptance clause names the lookup that discharged it on the verdict line, and the judgement is still a lookup rather than a reading",
        red_fires: red,
        green_passes: green,
    }
}

/// air-3xww: the round log is assembled from the coordinator's memory of messages, and a
/// coordinator that hits a limit, compacts or ends loses it. One did on 2026-09-06.
///
/// The gap is narrower than "agents should keep logs", which is why this is a directory and a
/// habit rather than a mechanism. That round already carried 40 digests, 37 captures and 196
/// agent-to-agent messages; what had nowhere to go was a finding that is neither about the bead
/// you hold nor worth the coordinator's inbox. A capture says somebody should act and every one
/// is triaged; these say nobody should.
///
/// **So this probe asserts the scaffolding and NOT the habit.** Air reads none of these files,
/// nothing refuses without one, and no condition counts them — a probe that checked a session
/// had written one would be the chore the bead rules out.
///
/// Red: `air init` scaffolds the journal's README under the configured directory, created only
/// when absent, exactly as the other scaffolded items are.
///
/// Green: the three things that keep it from becoming a mechanism. The directory is CONFIGURED,
/// so a repo that names another gets that one and no hard-coded path survives; a present README
/// is left alone; and `journal_dir` is absent from every gate fact, so no refusal can depend on
/// it.
fn probe_the_journal_is_scaffolded_and_nothing_reads_it() -> Probe {
    use crate::cmd::init::{DEFAULT_JOURNAL_DIR, scaffold};

    let fresh = scaffold(None, false, (DEFAULT_JOURNAL_DIR, false));
    let readme = format!("{DEFAULT_JOURNAL_DIR}/README.md");
    let scaffolds = fresh
        .iter()
        .any(|i| i.path == readme && i.create && i.note.contains("nothing in Air reads them"));
    // Alongside the others, not instead of them: the bead asks for it where `digest_dir`'s
    // neighbours are.
    let alongside = fresh.iter().any(|i| i.path == "Makefile" && i.create)
        && fresh
            .iter()
            .any(|i| i.path == ".worktreeinclude" && i.create);

    // Configured, not hard-coded.
    let elsewhere = scaffold(None, false, ("log.d", false));
    let honours_config = elsewhere
        .iter()
        .any(|i| i.path == "log.d/README.md" && i.create)
        && !elsewhere
            .iter()
            .any(|i| i.path.starts_with(DEFAULT_JOURNAL_DIR));
    // Present is present: created only when absent, the rule every scaffolded item follows.
    let untouched = scaffold(None, false, (DEFAULT_JOURNAL_DIR, true))
        .iter()
        .any(|i| i.path == readme && !i.create);

    // Nothing gates on it: the gate's facts carry no journal, so no refusal can read one.
    let gate_facts =
        serde_json::to_string(&air_hooks::gate::GateFacts::default()).unwrap_or_default();
    let no_gate = !gate_facts.contains("journal");

    Probe {
        name: "journal: air init scaffolds the session journal where the other scaffolded items go, at the configured path, and no gate reads it",
        red_fires: scaffolds && alongside,
        green_passes: honours_config && untouched && no_gate,
    }
}

/// air-kexg: a session journal commit names no bead, and a branch of them could not land.
///
/// The journal (air-3xww) is per session, ungated, and explicitly not work on a bead, so a
/// journal commit is the one commit a worker legitimately writes that names none. **Two
/// workers concluded independently that such a branch could land, by different reasoning** —
/// alerts from air-7kp's rule that a trailerless commit attributes to nothing, verify from
/// having no bead in flight — and no surface a worker can reach said otherwise. The
/// coordinator's workaround was to amend with `Bead: air-3xww`, honest while that bead was
/// hours old and a lie the moment it was not.
///
/// Red (declared mutation: the permitting half always answers false): a journal-only range is
/// refused again, which is the defect.
///
/// Green is mostly the CONSTRAINT, because that is the half that would still look right if it
/// rotted: a range mixing journal commits with anything else needs a bead exactly as before,
/// in either order and however lopsided; a repo declaring no `journal_dir` has no journal case
/// at all; an empty range is not journal-only; and a path that merely starts with the
/// directory's name (`docs/journalism/x.md` against `docs/journal`) is not inside it. A
/// declared field is read, never a guess about which paths look like a journal.
fn probe_a_journal_only_branch_needs_no_bead() -> Probe {
    use crate::cmd::status::journal_only;

    let j = Some("docs/journal");
    let entry = "docs/journal/alerts-22.md".to_string();
    let other = "docs/journal/ledger-2c.md".to_string();
    let work = "crates/cli/src/cmd/land.rs".to_string();

    // RED: the case the bead is about — a branch of journal entries alone.
    let red = journal_only(std::slice::from_ref(&entry), j)
        && journal_only(&[entry.clone(), other.clone()], j);

    // THE CONSTRAINT. A range that mixes still needs a bead, whichever way round and however
    // lopsided; this is a name for one legitimate commit, never a bypass for work that forgot
    // its trailer.
    let mixed = !journal_only(&[entry.clone(), work.clone()], j)
        && !journal_only(&[work.clone(), entry.clone()], j)
        && !journal_only(std::slice::from_ref(&work), j);
    // A repo that declares no journal_dir has no journal case; nothing changes for it.
    let undeclared = !journal_only(std::slice::from_ref(&entry), None)
        && !journal_only(std::slice::from_ref(&entry), Some(""));
    // An empty range is not journal-only: nothing to land is a different answer.
    let empty = !journal_only(&[], j);
    // A prefix is not a parent. `docs/journalism` is not inside `docs/journal`.
    let not_a_prefix = !journal_only(&["docs/journalism/x.md".to_string()], j)
        && !journal_only(&["docs/journal.md".to_string()], j);
    // A trailing slash in the declared value means the same directory.
    let slash_tolerant = journal_only(std::slice::from_ref(&entry), Some("docs/journal/"));

    let green = mixed && undeclared && empty && not_a_prefix && slash_tolerant;
    Probe {
        name: "land: a branch whose only commits are session-journal entries lands with no bead, and a range mixing them with anything else still needs one",
        red_fires: red,
        green_passes: green,
    }
}

/// air-72t7: `air status --json` said which branches can land and never why the others cannot.
///
/// `select` computes all three of `landings`, `skipped` and `errors` — air-6u5 added the last
/// two precisely so nothing is silent — and the snapshot took only the first, through a helper
/// that existed to discard the other two. So a `select()` error the code deliberately raises
/// rather than defaulting reached no caller at all, and `landable: []` was a well-formed answer
/// with nothing in it saying "I could not tell".
///
/// **Same class as the bug it came from, one layer up**: the promise is real and lives on
/// `select`; the surface a reader actually looks at dropped the half that keeps it.
///
/// Found by USE, not by reading — verify wrote another bead's probe against `--json` and could
/// not assert on `skipped`, because the field did not exist. And the incident it explains is
/// alerts', whose own account is the reason the shape matters: they read `landable: []`,
/// believed it, and spent three actions that could not have helped, because an empty list gives
/// a reader nothing to disbelieve. What ended it was reading the Rust — which is not available
/// to most readers of `--json`.
///
/// Red: the snapshot carries `skipped` and `errors`, and both survive into the JSON a caller
/// parses.
///
/// Green: each skip **names what was compared, not only the verdict** (alerts' caution, and
/// air-rud0's finding one surface over: a reason-shaped string can be true-sounding and wrong
/// when the comparison was never made). So `green-at-head` carries the head it looked at, and
/// `no-bead-named` carries the range it searched. And an error is distinguishable from an
/// absence: a `Selection` with an error and no landings serialises with the error present, so
/// `landable: []` can no longer be read as "nothing to land" when it means "I could not tell".
fn probe_status_json_says_why_a_branch_cannot_land() -> Probe {
    use crate::cmd::status::{Selection, Skipped, Snapshot};

    let skipped = |check: &'static str, detail: &str| Skipped {
        worker: "alerts".into(),
        check,
        detail: detail.into(),
        fix: "a fix".into(),
    };
    let selection = Selection {
        landings: Vec::new(),
        skipped: vec![
            skipped(
                "green-at-head",
                "alerts has no recorded green at its head de0f19cf",
            ),
            skipped(
                "no-bead-named",
                "alerts is green at de0f19cf but no commit in main..de0f19cf declares a bead",
            ),
        ],
        errors: vec!["alerts: git rev-parse HEAD: boom".to_string()],
    };
    let snap = Snapshot {
        landable: selection.landings.clone(),
        land_skipped: selection.skipped.clone(),
        land_errors: selection.errors.clone(),
        ..Default::default()
    };

    // RED drives the real path: a repo with a worktree that cannot land, `air --json status`,
    // and the reason parsed back out of the JSON a caller actually reads. A `Snapshot` built by
    // hand never touches the boundary between `select` and the snapshot, which is where the two
    // fields were being dropped — that anchor would mutate code this probe does not exercise,
    // which is air-682's wrong-path trap.
    let carried = (|| -> Option<bool> {
        let exe = std::env::current_exe().ok()?;
        let root = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        let main = root.join("main");
        let wt = root.join("worktree-alpha");
        std::fs::create_dir_all(main.join(".air")).ok()?;
        let g = |args: &[&str]| crate::git::run(&main, args).ok();
        g(&["init", "-q", "-b", "main", "."])?;
        g(&["config", "user.email", "a@b"])?;
        g(&["config", "user.name", "a"])?;
        std::fs::write(main.join("f"), "x").ok()?;
        g(&["add", "-A"])?;
        g(&["commit", "-qm", "seed"])?;
        // A worktree with no recorded green: `select` must skip it with `green-at-head`, and
        // the JSON must carry that.
        g(&[
            "worktree",
            "add",
            "-q",
            "-b",
            "alpha",
            &wt.to_string_lossy(),
        ])?;
        let out = air_command(&exe, &wt)
            .args(["--json", "status"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let _ = std::fs::remove_dir_all(&root);
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        let snapshot = v.get("snapshot")?;
        let skipped = snapshot.get("land_skipped")?.as_array()?;
        Some(
            snapshot.get("land_errors").is_some()
                && skipped.iter().any(|s| {
                    s.get("check").and_then(serde_json::Value::as_str) == Some("green-at-head")
                        // and it names WHAT IT COMPARED, not only the verdict
                        && s.get("detail")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|d| d.contains("head"))
                }),
        )
    })()
    .unwrap_or(false);

    // Green: the values compared are in the reason, not only the verdict.
    let names_what_it_compared = snap
        .land_skipped
        .iter()
        .find(|s| s.check == "green-at-head")
        .is_some_and(|s| s.detail.contains("de0f19cf"))
        && snap
            .land_skipped
            .iter()
            .find(|s| s.check == "no-bead-named")
            .is_some_and(|s| s.detail.contains("main..de0f19cf"));
    // An error is not an absence: empty landings plus an error is a distinguishable state.
    let error_is_not_absence = snap.landable.is_empty() && !snap.land_errors.is_empty();
    // And a clean fleet says nothing, so this costs a healthy round no output.
    let quiet = {
        let ok = Snapshot::default();
        ok.land_skipped.is_empty() && ok.land_errors.is_empty()
    };

    Probe {
        name: "status: --json says why each branch cannot land and distinguishes an error from an empty queue",
        red_fires: carried,
        green_passes: names_what_it_compared && error_is_not_absence && quiet,
    }
}

/// air-33rn: two workers concluded independently that a journal-only branch could land, both
/// were wrong, and **neither surface a worker can reach said so.** Each learned it from the
/// coordinator running `air land`, which workers are denied.
///
/// The fix is a read, not a second copy of the landing decision. `select` already computes the
/// whole answer as `Skipped { check, detail, fix }`, so `landing_line` looks the worker up in
/// that answer and renders it. Restating the decision is air-avj's shape and this round has hit
/// it three times; the rule there is to name what knows rather than repeat it.
///
/// **Pure over a `Selection`, with no fixture at all**, which is deliberate. alerts hit exactly
/// this on air-kexg: a shared fixture whose worker already carried work, so the range was
/// genuinely mixed and the test proved the constraint while claiming to prove the permission.
/// Most fixtures here carry work. Constructing the selection removes the possibility.
///
/// Red: a worker whose branch `select` skipped is told so, with the check, the detail and the
/// fix — the journal-only case, and the no-bead-named case, each without the coordinator.
///
/// Green: the three answers that are not a refusal. A landable branch says so with what it
/// carries; a worker selection never considered is told that rather than told "not landable",
/// which would be a verdict nothing reached; and an error about that worker outranks both,
/// because "cannot tell" and "no" are different and air-6u5 exists over that difference.
fn probe_handover_says_what_the_landing_gate_would_say() -> Probe {
    use crate::cmd::handover::landing_line;
    use crate::cmd::status::{Landing, Selection, Skipped};

    let skipped = |worker: &str, check: &'static str, detail: &str| Skipped {
        worker: worker.into(),
        check,
        detail: detail.into(),
        fix: "add a `Bead: <id>` trailer".into(),
    };
    let sel = Selection {
        landings: vec![Landing {
            worker: "alpha".into(),
            bead: Some("air-1".into()),
            head: "aaaaaaaa1111".into(),
            minutes: 3,
            command: "air land --worker alpha".into(),
            acceptance: Vec::new(),
            blocked: None,
        }],
        skipped: vec![
            skipped(
                "ledger",
                "no-bead-named",
                "ledger is green at aa2a9613 but no commit in main..aa2a9613 declares a bead",
            ),
            skipped(
                "beta",
                "green-at-head",
                "beta has no recorded green at its head bbbb1111",
            ),
        ],
        errors: vec!["gamma: git rev-parse HEAD: boom".to_string()],
    };

    // The journal-only branch: the case that cost two workers a round.
    let journal = landing_line(&sel, "ledger");
    // And a second precondition, so this is not one string's worth of coverage.
    let nogreen = landing_line(&sel, "beta");
    let red = journal.contains("NOT landable")
        && journal.contains("no-bead-named")
        && journal.contains("main..aa2a9613")
        && journal.contains("Bead: <id>")
        && nogreen.contains("NOT landable")
        && nogreen.contains("green-at-head");

    // Landable says so, with what it carries.
    let ok = landing_line(&sel, "alpha");
    // Never considered is not the same as refused.
    let absent = landing_line(&sel, "nobody");
    // "Cannot tell" outranks both: an error must never read as a verdict.
    let broke = landing_line(&sel, "gamma");
    let green = ok.contains("landable at aaaaaaaa")
        && ok.contains("air-1")
        && !ok.contains("NOT landable")
        && absent.contains("not a landing candidate")
        && !absent.contains("NOT landable")
        && broke.contains("cannot tell")
        && broke.contains("boom");

    Probe {
        name: "handover: a worker is told what the landing gate would say about its own branch, read from select rather than recomputed",
        red_fires: red,
        green_passes: green,
    }
}

/// air-htmn: `air land` told an adopter's coordinator "could not fast-forward main onto the
/// landing commit, so main is untouched … git timed out after 1.5s". **Main was at the landing
/// commit.** `git merge --ff-only` updates the ref atomically and `git::run` kills the child on
/// expiry, which does not undo a ref update — so an `Err` there means "I stopped waiting", never
/// "it did not happen". They ran `air land` again, and the second call's correct refusal is the
/// only reason they learned the first had worked.
///
/// **The worse half was the row.** `record("refused", …, "fast-forward")` ran on the same
/// branch, so the ledger said refused for a landing that happened, and `landings()`,
/// `landed_open()` and `air status` all read that row afterwards. A wrong sentence is read once;
/// a wrong row is read by every mechanism standing on the record.
///
/// Air does not have to say "I do not know" here — it can look, and `merge-base --is-ancestor`
/// settles it. Raising the 1500 ms constant would narrow the window and leave the inference
/// exactly as unsound, which is why the fix is the look and not the number.
///
/// Third arrival of this round's pattern, after air-rud0 and air-k6uh: a lookup that established
/// something weaker than the sentence built on it.
///
/// Red: a fast-forward that completed reads as LANDED despite the error — and the ancestry the
/// decision reads is answered by git against a real repo whose main is at the landing commit,
/// not by me, so this is not a probe testing its own constructor (the bead asked for that
/// distinction explicitly).
///
/// Green: the two cases that must not be swept up with it. Main genuinely not moved still
/// refuses, with its existing text. And when the look itself fails, neither is established: the
/// message says so and **nothing is recorded**, because writing "refused" there would be this
/// same bug one layer over.
fn probe_a_timed_out_fast_forward_is_not_reported_as_untouched() -> Probe {
    use crate::cmd::land::{FfVerdict, after_fast_forward};

    let timeout = "git timed out after 1.5s";

    // The ancestry input comes from git, against a repo where main really is at the merge.
    let observed: Option<bool> = (|| {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).ok()?;
        let g = |args: &[&str]| crate::git::run(&dir, args).ok();
        g(&["init", "-q", "-b", "main", "."])?;
        g(&["config", "user.email", "a@b"])?;
        g(&["config", "user.name", "a"])?;
        std::fs::write(dir.join("f"), "x").ok()?;
        g(&["add", "-A"])?;
        g(&["commit", "-qm", "seed"])?;
        let merge = g(&["rev-parse", "HEAD"])?;
        let answer = crate::git::is_ancestor(&dir, &merge, "HEAD").ok();
        let _ = std::fs::remove_dir_all(&dir);
        answer
    })();

    let landed = after_fast_forward(timeout, observed);
    let red = observed == Some(true) && landed == FfVerdict::Landed;

    let refused = after_fast_forward(timeout, Some(false));
    let unknown = after_fast_forward(timeout, None);
    let green = matches!(&refused, FfVerdict::Refused(m)
            if m.contains("main is untouched") && m.contains(timeout))
        && matches!(&unknown, FfVerdict::Unknown(m)
            if m.contains("NOT established")
                && m.contains("nothing was recorded")
                && m.contains(timeout))
        // Unknown is not a refusal wearing a different word: the three are distinct.
        && refused != unknown
        && landed != refused;

    Probe {
        name: "land: a fast-forward that completed is not reported as untouched, and a look that failed is not reported as a refusal",
        red_fires: red,
        green_passes: green,
    }
}
