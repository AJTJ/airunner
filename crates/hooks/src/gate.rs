//! The one refusal: hand-over (plan 0001 §4).
//!
//! A pure function of facts — no I/O here, so it is trivially testable and its decision is a
//! deterministic function of (ledger rows, git state). The CLI/hook gathers `GateFacts` and
//! then calls [`handover_verdict`]. Every refusal names what is missing and the command that
//! fixes it (workers' ask: "print the command that fixes it").

use serde::Serialize;

/// Facts the gate needs. All are observations, none are model text.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GateFacts {
    pub worker: String,
    pub head: String,
    /// `verify_runs` has a green `verify` row that stands for `head` under the repo's key
    /// (air-7wf): at the commit itself by any worker, or at its tree where the repo declares
    /// `verify_key: tree`.
    pub green_at_head: bool,
    /// A green exists for `head`'s exact tree but does not count under the repo's key
    /// (air-7wf). Only for the message: it names the re-verify as the price of a
    /// commit-keyed repo rather than as an absence.
    pub tree_green: Option<String>,
    /// A green recorded at a verified commit that contains every commit of the bead(s) being
    /// handed over, and the main THAT RUN WAS RECORDED OVER (air-80x.1, air-9ij): the verify
    /// lane's batch. Counts as green. Main moving afterwards does not retract it — that is the
    /// landing gate's question, not this one. Also carries the landed case, where the bead has
    /// no commit outside main because Air landed it and the landing required a green.
    pub batch_green: Option<String>,
    /// A batch green that contains main and some of the bead's commits but not the newest:
    /// the batch predates the worker's last commit (air-80x.1). For the refusal only.
    pub batch_predates: Option<String>,
    /// The most recent green sha for this worker, if any (for the message).
    pub last_green_sha: Option<String>,
    /// `git merge-base --is-ancestor main HEAD`.
    pub main_is_ancestor: bool,
    /// `git rev-parse main` at the moment the facts were read (air-5wq). Empty when unknown.
    pub main_sha: String,
    /// When main is not an ancestor: the landing that moved it past this branch, if the
    /// ledger has one (air-4up). The external cause the refusal names.
    pub main_moved: Option<MainMove>,
    /// The named bead is this worker's to hand over: it holds an open claim on it, OR a
    /// commit in `main..HEAD` declares it in a `Bead:` trailer (air-60x). The trailer is the
    /// same fact `air land` reads to decide which beads a branch carries, so the two agree on
    /// what makes a branch handable; before this the gate consulted claims alone, and Air
    /// would LAND a branch it refused to let its author HAND OVER. A branch that supersedes
    /// another worker's closed bead carries it by trailer and can hold no claim on it.
    pub bead_claimed_or_carried: bool,
    /// The bead NAMED to the gate: `--bead` on the CLI, or the id in the `bd` command on the
    /// hook path. `None` when nothing was named.
    pub bead: Option<String>,
    /// Every bead this worker holds an open claim on, from the ledger (air-xbl). This is what
    /// `air status` prints under `claims:`, and it is what a refusal names when no bead was
    /// named: the id was already computed for the digest lookup and then thrown away, so
    /// the fixing command printed a literal placeholder a worker could not run.
    pub held_beads: Vec<String>,
    /// Every bead a commit in `main..HEAD` declares in a `Bead:` trailer (air-60x): the work
    /// this branch carries, whoever claimed it. What `air land` will attribute the landing to.
    pub carried_beads: Vec<String>,
    /// (green, red) runs recorded at HEAD; disagreement is reported as flakiness.
    pub runs_at_head: (i64, i64),
    /// Digest check (owner ruling D, 2026-08-21): `None` when the repo configures no digest
    /// directory, or when there is no bead to declare (no bead named and no claim held,
    /// air-xbl: a batching lane that merges other workers' green work holds nothing and hands
    /// over); `Some(false)` when no digest declares a bead this worker is handing over.
    pub digest_present: Option<bool>,
    /// A digest declaring the bead EXISTS in the directory but git does not track it
    /// (air-ahl). Only reachable alongside `digest_present: Some(false)`, and only for the
    /// message: "write one" and "add the one you wrote" are different fixes, and a worker told
    /// the first while looking at the second learns to distrust the gate.
    pub digest_untracked: bool,
    /// Where digests live (for the fixing message).
    pub digest_dir: Option<String>,
    /// Advisory mode: report what would be refused but allow (first round; decisions.md).
    pub advisory: bool,
}

/// A landing that moved main (air-4up). The adopter, 2026-08-30: eight refusals in one
/// round, all four workers, every one caused by a coordinator landing, and the wording
/// described the worker's tree. Two workers read it as their own defect and merged again
/// without asking why. The instruction was right and the diagnosis was misleading.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MainMove {
    pub merge_commit: String,
    /// The worker whose branch landed.
    pub worker: String,
    pub at: String,
    /// Seconds between the landing and the facts being read; None when either clock is
    /// unreadable.
    pub ago_secs: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verdict {
    /// True when every condition holds.
    pub pass: bool,
    /// Whether the caller should actually block (false in advisory mode even when !pass).
    pub block: bool,
    pub missing: Vec<Missing>,
    /// One human line, suitable for stderr / additionalContext.
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Missing {
    pub check: &'static str,
    pub detail: String,
    pub fix: String,
    /// This fix depends on the repo's own WORK FLOW, not just on the facts (air-avj).
    ///
    /// Merging main and recording a verify are the right repair with no verify lane and are
    /// the two things a lane exists to stop a worker doing: merging moves the head off the sha
    /// the lane cut at, and recording a green is the lane's job. An adopter's Stop hook printed
    /// both to three workers under a lane in one round — w1 obeyed and lost its batch
    /// membership, w2 ignored it at the cost of a round trip, w3 asked instead of obeying.
    ///
    /// The flag exists so [`stop_message`] can drop these repairs and name the command that
    /// computes them. It is set beside each `fix` rather than derived from a list of check
    /// names, so a new check has to decide rather than default into being advertised at Stop
    /// time.
    ///
    /// **air-155w: the flag governs the fix TEXT as well, not only who prints it.** air-avj
    /// changed what the Stop hook prints and left the refusal itself asserting
    /// `git merge main && air record verify -- make verify`, so an adopter's w1 read the
    /// forbidden clause from the refusal instead. That is the nastiest form of this defect:
    /// obeying the first clause and ignoring the second is exactly right, so following the
    /// line WORKS, the worker gets a good outcome and learns the wrong habit, and nothing ever
    /// contradicts it. A fix that failed outright would have been found in one use.
    ///
    /// So a `flow_dependent` fix now states **what must become true**, not which command a
    /// particular flow uses to make it true. `verify_lane` is still read nowhere: the repo's
    /// flow decides who runs the verify, and Air names the condition either way.
    pub flow_dependent: bool,
}

pub fn handover_verdict(f: &GateFacts) -> Verdict {
    let mut missing = Vec::new();
    // air-80x.1: a verify lane's batch green counts when it contains main and every commit
    // of the bead. It is a second way to be green, never a way to be less than green.
    let green = f.green_at_head || f.batch_green.is_some();
    if !green {
        let last = f
            .last_green_sha
            .as_deref()
            .map(|s| format!(" (last green: {s})"))
            .unwrap_or_default();
        let (g, r) = f.runs_at_head;
        let (detail, fix) = if let Some(p) = f.batch_predates.as_deref() {
            (
                format!("no green verify recorded at HEAD {}; {p}", short(&f.head)),
                "wait for the lane's next batch to cover this commit".to_string(),
            )
        } else if g > 0 && r > 0 {
            (
                format!(
                    "verify at HEAD {} is flaky: {g} green / {r} red; latest is red",
                    short(&f.head)
                ),
                "fix or quarantine the flaky test (file it), then get a green at this head"
                    .to_string(),
            )
        } else if let Some(tree) = f.tree_green.as_deref() {
            (
                format!(
                    "no green verify recorded at HEAD {}; {tree}",
                    short(&f.head)
                ),
                "a green at this head; `air handover` names what your flow needs to \
                 produce one"
                    .to_string(),
            )
        } else {
            (
                format!("no green verify recorded at HEAD {}{last}", short(&f.head)),
                "a green at this head; `air handover` names what your flow needs to \
                 produce one"
                    .to_string(),
            )
        };
        missing.push(Missing {
            check: "verify-green-at-head",
            detail,
            fix,
            // Recording a verify is the lane's job, never the worker's, where one runs.
            flow_dependent: true,
        });
    }
    if !f.main_is_ancestor {
        // air-4up: the cause is outside the worker's tree, so say so. "main is not an
        // ancestor of HEAD" stays in every form: the adopter's counts refusals by that phrase.
        // The fix is unchanged; this is wording, not behaviour.
        let at_main = if f.main_sha.is_empty() {
            String::new()
        } else {
            format!("; main is at {}", short(&f.main_sha))
        };
        let detail = if let Some(m) = f.main_moved.as_ref() {
            format!(
                "main is not an ancestor of HEAD {}: your branch is behind main, which moved \
                 {} to {} (landing from {}){at_main}",
                short(&f.head),
                ago(m.ago_secs),
                short(&m.merge_commit),
                m.worker
            )
        } else {
            format!(
                "main is not an ancestor of HEAD {}{at_main}",
                short(&f.head)
            )
        };
        missing.push(Missing {
            check: "main-merged",
            detail,
            // air-155w: merging IS right and necessary under both flows — the adopter's
            // report says so — and recording a green is the one clause a lane forbids. So the
            // merge stays a command and the green becomes a condition: whose job it is to
            // produce one is the repo's flow to say, not this line's.
            fix: "git merge main, then a green at the new head (your own, or your lane's)"
                .to_string(),
            // Merging moves the head off the sha a lane cut its batch at.
            flow_dependent: true,
        });
    }
    // air-xbl: the ids a refusal names. The named bead first; else every bead the worker
    // holds or carries. Never a placeholder: the printed fix is the one line in the flow a
    // worker copies verbatim, and the adopter's w3 was handed `air claim <bead>` while holding
    // one.
    let beads = beads_to_name(f);
    if !f.bead_claimed_or_carried {
        // air-60x: the fix is the trailer, never `air claim <id>`. A claim reserves OPEN work
        // and is the roles flow's business; the bead named here may be closed and another
        // worker's (supersession), and claiming a closed bead is not a fix. The adopter was
        // right to refuse to test that command rather than let the gap read as cleared.
        let (detail, fix) = match beads.as_slice() {
            [bead] => (
                format!(
                    "{bead} is neither claimed by {} nor named by a `Bead:` trailer in main..HEAD",
                    f.worker
                ),
                format!(
                    "commit its work with a `Bead: {bead}` trailer (git commit --amend); that is what `air land` reads too"
                ),
            ),
            [] => (
                format!("no bead is claimed by or carried on {}'s branch", f.worker),
                "commit the work with a `Bead: <id>` trailer naming the bead it does".to_string(),
            ),
            many => (
                format!(
                    "none of {} is claimed by {} or named by a `Bead:` trailer in main..HEAD",
                    many.join(", "),
                    f.worker
                ),
                format!(
                    "commit the work with a `Bead:` trailer naming it, one of {}",
                    many.join(", ")
                ),
            ),
        };
        missing.push(Missing {
            check: "claim",
            detail,
            fix,
            // A `Bead:` trailer is the trailer whatever the repo's flow is.
            flow_dependent: false,
        });
    }
    if f.digest_present == Some(false) {
        let dir = f.digest_dir.as_deref().unwrap_or("docs/log.d");
        // air-ahl: written but never tracked. An adopter's worker used this deliberately and
        // reported it anyway: the file satisfied the gate while existing for nobody but that
        // worktree, so a green said nothing about whether a digest would exist for the next
        // reader. Named apart from "no digest", because the fixes are different sentences.
        if f.digest_untracked {
            let one = beads.first().cloned().unwrap_or_default();
            missing.push(Missing {
                check: "digest-untracked",
                detail: format!(
                    "a digest in {dir} declares `bead: {one}` but git does not track it, so it \
                     exists for nobody but this worktree"
                ),
                fix: format!(
                    "git add {dir} && git commit -m \"docs: digest for {one}\" — and if your \
                     lane has already cut a batch at this head, that commit must carry NO \
                     `Bead:` trailer: the batch green has to contain every commit that NAMES \
                     the bead, and an untrailered digest commit never joins that set, so your \
                     head moves and the close still passes at the batch you were cut at"
                ),
                // Committing a digest is the same act under either flow; only the trailer
                // advice is about the batching one, and it is guarded by its own sentence.
                flow_dependent: false,
            });
        } else {
            // air-agq: the gate reads a declared `bead:` field, so the fix has to name it.
            // Saying "write a digest" was true of the old filename guess and would leave a
            // worker with a written digest and a gate that still refuses.
            let (detail, fix) = match beads.as_slice() {
                [bead] => (
                    format!("no digest in {dir} declaring `bead: {bead}`"),
                    format!(
                        "write {dir}/<date>-{}-{bead}.md opening with front matter:\n---\nbead: {bead}\n---",
                        f.worker
                    ),
                ),
                // Unreachable from `handover::facts`, which skips the check when there is no
                // bead to declare; a caller that sets the fact by hand still gets no placeholder.
                [] => (
                    format!(
                        "no digest in {dir}, and no bead to declare: {} holds no claim and none was named",
                        f.worker
                    ),
                    "air claim the bead first, then write its digest with `bead:` front matter"
                        .to_string(),
                ),
                many => (
                    format!(
                        "no digest in {dir} declaring any of {}",
                        many.iter()
                            .map(|b| format!("`bead: {b}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    format!(
                        "write {dir}/<date>-{}-<one of {}>.md opening with front matter naming that bead:\n---\nbead: {}\n---",
                        f.worker,
                        many.join("|"),
                        many.join("|")
                    ),
                ),
            };
            // air-yol: the fix above is right and following it created the next refusal.
            // Committing the digest moves HEAD off the sha the green was recorded at, so the very
            // next check refuses on verify-green-at-head (the adopter, two workers hit it
            // independently on 2026-08-31). Say so HERE, where a worker is reading, and only when
            // there is a green to invalidate: with no green at HEAD the order is already the
            // standard one and the note would be the unconditional noise air-5wq was left open
            // over.
            let order_note = if f.green_at_head {
                format!(
                    "\nthen commit it and merge main BEFORE the green is taken: committing the \
                 digest moves HEAD off {}, where the green is recorded, and the next check \
                 would refuse for a green that is no longer at HEAD. Whoever your flow has \
                 record that green (you, or your lane) must do it last",
                    short(&f.head)
                )
            } else {
                String::new()
            };
            missing.push(Missing {
                check: "digest-present",
                detail,
                fix: format!("{fix}{order_note}"),
                // Where a repo configures digests, writing one is the same act under either
                // flow.
                flow_dependent: false,
            });
        }
    }
    let pass = missing.is_empty();
    let block = !pass && !f.advisory;
    let message = if pass {
        // air-5wq: a snapshot that reads as a clearance. The adopter measured 88 refusals in
        // four days arriving within 120 s of that same worker's own `handover ok`: an answer
        // expiring before it could be used. Naming the main it was true of lets a reader see
        // at a glance whether it still applies, and the refusal (which names main too, air-4up)
        // then reads as main having moved rather than as a contradiction.
        // air-80x.1: when the green is a batch's, say whose and what it contains.
        let batch = f
            .batch_green
            .as_deref()
            .filter(|_| !f.green_at_head)
            .map(|b| format!("; {b}"))
            .unwrap_or_default();
        format!(
            "handover ok: {} at {}{}{batch}",
            f.worker,
            short(&f.head),
            containing_main(&f.main_sha)
        )
    } else {
        let mode = if f.advisory {
            "would refuse"
        } else {
            "refused"
        };
        let items: Vec<String> = missing
            .iter()
            .map(|m| format!("{}: {} — run `{}`", m.check, m.detail, m.fix))
            .collect();
        // air-75u: whose tree, as the ok line already says. A refusal shown in a session that
        // is not the one it is about is otherwise a true statement with no scope.
        format!(
            "handover {mode} for {} at {}: {}",
            f.worker,
            short(&f.head),
            items.join("; ")
        )
    };
    Verdict {
        pass,
        block,
        missing,
        message,
    }
}

/// What the STOP HOOK says about a refusal (air-avj): the same facts, with the flow-dependent
/// repairs replaced by the command that computes them.
///
/// The hook fires when a worker is choosing what to do next, and it arrives with the authority
/// of tooling. Under a verify lane its old text told three of an adopter's workers to do the
/// two things a lane exists to prevent — merge main, which moves the head off the sha the lane
/// cut at, and record a green, which is the lane's job. One obeyed and lost its batch
/// membership.
///
/// **The decision, recorded because it was a decision** (the bead offered three routes). Air
/// does NOT read `verify_lane`: that key is the repo's own, for choosing its own flow, and a
/// hook branching on it would be a second copy of a decision `air handover` already makes
/// correctly — which is the drift that made this wrong rather than merely unhelpful. Nor is
/// the wording made vague enough to be true under both, because the two repairs are opposite
/// actions and a sentence covering both says nothing. Instead the hook states the fact and
/// names the command that knows: `air handover` reads the repo's own configuration and prints
/// the repair its flow calls for.
///
/// **What does not change.** `air handover` and every CLI surface still print the exact
/// repair; a fix that does not depend on the flow — the `Bead:` trailer, the digest — is still
/// printed here in full. Only the two flow-dependent ones become a pointer, and only at Stop.
///
/// Generalises past this case: any surface that restates a flow-dependent repair drifts from
/// the one place that computes it. Name the command that knows.
pub fn stop_message(v: &Verdict, worker: &str, head: &str) -> String {
    if v.pass {
        return v.message.clone();
    }
    let mode = if v.block { "refuses" } else { "would refuse" };
    let items: Vec<String> = v
        .missing
        .iter()
        .map(|m| {
            if m.flow_dependent {
                format!("{}: {}", m.check, m.detail)
            } else {
                format!("{}: {} — run `{}`", m.check, m.detail, m.fix)
            }
        })
        .collect();
    // air-155w: this used to say `air handover` "reads your repo's flow", which Air does
    // not do and deliberately does not do — `verify_lane` is read nowhere. The promise was
    // the same defect one surface over: a claim about a decision made somewhere else. What
    // is true is that `air handover` prints every check in full, and since that text is now
    // flow-safe too, the pointer no longer has to promise anything about flows.
    let pointer = if v.missing.iter().any(|m| m.flow_dependent) {
        " — run `air handover` for each check and what it needs"
    } else {
        ""
    };
    format!(
        "handover {mode} for {worker} at {}: {}{pointer}",
        short(head),
        items.join("; ")
    )
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// The ids a refusal may name: the bead named to the gate, else every bead the worker holds
/// or its branch carries (air-xbl, air-60x). Empty only when none exists.
fn beads_to_name(f: &GateFacts) -> Vec<String> {
    match &f.bead {
        Some(b) => vec![b.clone()],
        None => {
            let mut v = f.held_beads.clone();
            for c in &f.carried_beads {
                if !v.contains(c) {
                    v.push(c.clone());
                }
            }
            v
        }
    }
}

/// ", containing main <sha>", or nothing when main's sha could not be read.
fn containing_main(main_sha: &str) -> String {
    if main_sha.is_empty() {
        String::new()
    } else {
        format!(", containing main {}", short(main_sha))
    }
}

/// "40s ago", "12 min ago", or "at an unknown time".
fn ago(secs: Option<i64>) -> String {
    match secs {
        Some(s) if s < 120 => format!("{s}s ago"),
        Some(s) => format!("{} min ago", s / 60),
        None => "at an unknown time".to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn facts() -> GateFacts {
        GateFacts {
            worker: "backend-leaning".into(),
            head: "f854145abcdef".into(),
            green_at_head: true,
            tree_green: None,
            batch_green: None,
            batch_predates: None,
            last_green_sha: Some("f854145abcdef".into()),
            main_is_ancestor: true,
            main_sha: "0a1b2c3d4e5f".into(),
            main_moved: None,
            bead_claimed_or_carried: true,
            runs_at_head: (1, 0),
            digest_present: None,
            digest_untracked: false,
            digest_dir: None,
            bead: Some("zz-o5fi".into()),
            held_beads: vec!["zz-o5fi".into()],
            carried_beads: vec![],
            advisory: false,
        }
    }

    /// air-80x.1: a batch green is a second way to be green; a batch that predates the last
    /// commit is named in the refusal with the fix that waits for the next batch.
    #[test]
    fn a_batch_green_passes_and_a_predating_batch_is_named() {
        let mut f = facts();
        f.green_at_head = false;
        f.batch_green =
            Some("green at abc12345 (batch by lane) contains every commit of zz-o5fi".into());
        let v = handover_verdict(&f);
        assert!(v.pass, "{}", v.message);
        assert!(v.message.contains("batch by lane"), "{}", v.message);

        let mut f = facts();
        f.green_at_head = false;
        f.batch_predates = Some("the batch's green at abc12345 (by lane) predates your commit deadbeef \"more work\" for zz-o5fi: it does not contain it".into());
        let v = handover_verdict(&f);
        assert!(v.block);
        assert!(
            v.missing[0]
                .detail
                .contains("predates your commit deadbeef"),
            "{}",
            v.missing[0].detail
        );
        assert!(
            v.missing[0].fix.contains("next batch"),
            "{}",
            v.missing[0].fix
        );
    }

    /// air-60x: the claim refusal never offers `air claim <id>`. The bead may be closed and
    /// another worker's; the fix is the trailer `air land` reads.
    #[test]
    fn the_claim_refusal_offers_the_trailer_never_a_claim() {
        let mut f = facts();
        f.bead = Some("zz-closed".into());
        f.held_beads = vec![];
        f.bead_claimed_or_carried = false;
        let v = handover_verdict(&f);
        let m = &v.missing[0];
        assert_eq!(m.check, "claim");
        assert!(m.detail.contains("zz-closed"), "{}", m.detail);
        assert!(m.fix.contains("Bead: zz-closed"), "{}", m.fix);
        assert!(!v.message.contains("air claim"), "{}", v.message);
    }

    /// air-xbl: the adopter's w3 held exactly one claim, had not written its digest, and was
    /// handed `bead: <bead>` to copy. The id was in the ledger and `air status` printed it;
    /// the refusal did not. With no bead named, the held beads are what the message names.
    #[test]
    fn a_refusal_names_the_held_bead_when_none_was_named() {
        let mut f = facts();
        f.bead = None;
        f.held_beads = vec!["zz-251z".into()];
        f.digest_present = Some(false);
        f.digest_dir = Some("docs/log.d".into());
        let v = handover_verdict(&f);
        let d = &v.missing[0];
        assert_eq!(d.check, "digest-present");
        assert!(d.detail.contains("bead: zz-251z"), "{}", d.detail);
        assert!(d.fix.contains("bead: zz-251z"), "{}", d.fix);
        assert!(!v.message.contains("<bead>"), "{}", v.message);

        // Several held: every id is named and none is invented.
        f.held_beads = vec!["zz-1".into(), "zz-2".into()];
        let v = handover_verdict(&f);
        assert!(
            v.message.contains("zz-1") && v.message.contains("zz-2"),
            "{}",
            v.message
        );
        assert!(!v.message.contains("<bead>"), "{}", v.message);

        // None held and none named: still no placeholder posing as a command.
        f.held_beads = vec![];
        f.bead_claimed_or_carried = false;
        let v = handover_verdict(&f);
        assert!(!v.message.contains("<bead>"), "{}", v.message);
        assert!(v.message.contains("holds no claim"), "{}", v.message);
    }

    /// The named bead wins over the held ones: on the hook path the id in the `bd` command is
    /// the bead being handed over, whatever else the worker holds.
    #[test]
    fn a_named_bead_is_what_the_refusal_names() {
        let mut f = facts();
        f.bead = Some("zz-named".into());
        f.held_beads = vec!["zz-other".into()];
        f.bead_claimed_or_carried = false;
        let v = handover_verdict(&f);
        assert!(
            v.missing[0].fix.contains("Bead: zz-named"),
            "{}",
            v.missing[0].fix
        );
        assert!(!v.message.contains("zz-other"), "{}", v.message);
    }

    #[test]
    fn flaky_head_is_named_in_the_refusal() {
        let mut f = facts();
        f.green_at_head = false;
        f.runs_at_head = (2, 1);
        let v = handover_verdict(&f);
        assert!(
            v.missing[0].detail.contains("flaky"),
            "{}",
            v.missing[0].detail
        );
        assert!(v.missing[0].fix.contains("quarantine"));
    }

    /// air-7wf: a green for this exact tree that the repo's key declines is named in the
    /// refusal, so a worker re-verifying after a fast-forward reads the price rather than an
    /// absence. It still refuses; the fix is unchanged.
    #[test]
    fn a_declined_tree_green_is_named_in_the_refusal() {
        let mut f = facts();
        f.green_at_head = false;
        f.tree_green =
            Some("this exact tree is green at 40076426 by w1, but keyed by commit".into());
        let v = handover_verdict(&f);
        assert!(v.block);
        assert!(
            v.missing[0].detail.contains("green at 40076426 by w1"),
            "{}",
            v.missing[0].detail
        );
        // air-155w: the fix names the condition, not a command a verify lane forbids.
        assert!(v.missing[0].fix.contains("a green at this head"));
        assert!(
            !v.missing[0]
                .fix
                .contains("air record verify -- make verify")
        );
    }

    /// air-4up: a refusal caused by a landing names the landing, when, and from whom, keeps
    /// the phrase the adopter counts by, and keeps the fix. Without a landing to name it still
    /// refuses and names main.
    #[test]
    fn a_refusal_after_a_landing_names_the_landing_that_moved_main() {
        let mut f = facts();
        f.main_is_ancestor = false;
        f.main_moved = Some(MainMove {
            merge_commit: "abcdef0123456".into(),
            worker: "lane".into(),
            at: "t".into(),
            ago_secs: Some(40),
        });
        let v = handover_verdict(&f);
        assert!(v.block);
        let m = &v.missing[0];
        assert_eq!(m.check, "main-merged");
        assert!(
            m.detail.contains("main is not an ancestor of HEAD"),
            "{}",
            m.detail
        );
        assert!(
            m.detail
                .contains("moved 40s ago to abcdef0 (landing from lane)"),
            "{}",
            m.detail
        );
        assert!(m.detail.contains("main is at 0a1b2c3"), "{}", m.detail);
        // air-155w: merging is required under both flows and stays a command; recording a
        // green is the clause a lane forbids and is now a condition.
        assert!(m.fix.starts_with("git merge main"));
        assert!(!m.fix.contains("air record verify"));

        f.main_moved = None;
        let v = handover_verdict(&f);
        assert!(v.block);
        assert!(!v.message.contains("landing"), "{}", v.message);
        assert!(v.message.contains("main is at 0a1b2c3"), "{}", v.message);
        assert_eq!(ago(Some(900)), "15 min ago");
        assert_eq!(ago(None), "at an unknown time");
    }

    #[test]
    fn all_green_passes() {
        let v = handover_verdict(&facts());
        assert!(v.pass && !v.block);
        assert!(v.message.starts_with("handover ok"));
    }

    /// air-5wq: the pair is the point. The ok line names the main it was true of; after main
    /// moves, the refusal names a different main, so the two read against each other rather
    /// than as a contradiction.
    #[test]
    fn the_ok_line_names_main_and_a_later_refusal_names_a_different_one() {
        let ok = handover_verdict(&facts());
        assert_eq!(
            ok.message,
            "handover ok: backend-leaning at f854145, containing main 0a1b2c3"
        );
        let mut f = facts();
        f.main_is_ancestor = false;
        f.main_sha = "9f9f9f9f9f9f".into();
        let refused = handover_verdict(&f);
        assert!(refused.block);
        assert!(
            refused.message.contains("main is at 9f9f9f9"),
            "{}",
            refused.message
        );
        assert!(!refused.message.contains("0a1b2c3"), "{}", refused.message);
        // An unreadable main is omitted, never rendered as an empty sha.
        let mut g = facts();
        g.main_sha = String::new();
        assert_eq!(
            handover_verdict(&g).message,
            "handover ok: backend-leaning at f854145"
        );
    }

    #[test]
    fn missing_verify_blocks_and_names_the_fix() {
        let mut f = facts();
        f.green_at_head = false;
        f.head = "9bb1713000".into();
        let v = handover_verdict(&f);
        assert!(!v.pass && v.block);
        assert_eq!(v.missing[0].check, "verify-green-at-head");
        // air-75u: the refusal names whose tree it is about, like the ok line.
        assert!(
            v.message
                .starts_with("handover refused for backend-leaning at 9bb1713: "),
            "{}",
            v.message
        );
        assert!(v.message.contains("last green: f854145"));
        // air-155w: the refusal names the check and the condition, never a command a verify
        // lane forbids that worker.
        assert!(v.message.contains("verify-green-at-head"));
        assert!(v.message.contains("a green at this head"));
        assert!(!v.message.contains("air record verify"));
    }

    #[test]
    fn advisory_reports_but_never_blocks() {
        let mut f = facts();
        f.green_at_head = false;
        f.main_is_ancestor = false;
        f.advisory = true;
        let v = handover_verdict(&f);
        assert!(!v.pass);
        assert!(!v.block, "advisory mode must not block");
        assert!(v.message.starts_with("handover would refuse"));
        assert_eq!(v.missing.len(), 2);
    }

    #[rstest]
    #[case(true, true, true, 0)]
    #[case(false, true, true, 1)]
    #[case(true, false, true, 1)]
    #[case(true, true, false, 1)]
    #[case(false, false, false, 3)]
    fn counts_missing(
        #[case] green: bool,
        #[case] main: bool,
        #[case] claimed: bool,
        #[case] n: usize,
    ) {
        let mut f = facts();
        f.green_at_head = green;
        f.main_is_ancestor = main;
        f.bead_claimed_or_carried = claimed;
        assert_eq!(handover_verdict(&f).missing.len(), n);
    }
}

/// Stop nudge (air-09i): a worker that holds no claim while beads are ready is told once
/// which beads are ready, by blocking its stop with the list. `stop_hook_active` is true
/// when Claude Code is already continuing because of a Stop hook (hooks reference,
/// https://code.claude.com/docs/en/hooks, accessed 2026-08-22; it also ends the turn after 8
/// consecutive blocks), so the nudge fires exactly once per stop, never a loop. The
/// coordinator is never nudged.
///
/// `ready` must be a list the caller has confirmed against live state, not a cached one
/// (air-ouw). There is no "may be stale" any more: the nudge either names beads `air claim`
/// will accept or says nothing. It used to name whatever the cache held, so a bead labelled
/// `owner` after the cache was written was offered here and refused by `air claim` seconds
/// later — the same fleet contradicting itself out of one stale file. An annotation admitting
/// a mechanism may be wrong is a mechanism that has not decided what it is for.
///
/// Removal condition (bead air-09i): when a round shows nudges that led to a claim <= nudges
/// ignored, or workers claim the next bead unprompted in > 90% of hand-overs.
pub fn stop_nudge(
    role: &str,
    holds_claim: bool,
    ready: &[String],
    stop_hook_active: bool,
) -> Option<String> {
    if role != "worker" || holds_claim || stop_hook_active || ready.is_empty() {
        return None;
    }
    let first = ready.first().map(String::as_str).unwrap_or_default();
    Some(format!(
        "air: ready: {}; claim one (air claim {first}) or say why you are stopping (air capture \"<why>\")",
        ready.join(", "),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod nudge_tests {
    use super::stop_nudge;

    fn ids() -> Vec<String> {
        vec!["zz-1".to_string(), "zz-2".to_string()]
    }

    #[test]
    fn nudges_only_a_claimless_worker_with_ready_beads_on_a_fresh_stop() {
        let r = stop_nudge("worker", false, &ids(), false).unwrap();
        assert!(r.contains("ready: zz-1, zz-2"));
        assert!(r.contains("air claim zz-1"));
        // air-ouw: there is no staleness caveat any more, in either direction. The caller
        // confirms the list against bd before this is reached, so the nudge either names
        // beads `air claim` accepts or is not called at all.
        assert!(!r.contains("stale"));
    }

    #[test]
    fn every_other_combination_passes() {
        assert!(stop_nudge("coordinator", false, &ids(), false).is_none());
        assert!(stop_nudge("worker", true, &ids(), false).is_none());
        assert!(stop_nudge("worker", false, &[], false).is_none());
        assert!(stop_nudge("worker", false, &ids(), true).is_none());
    }
}
