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
    /// The most recent green sha for this worker, if any (for the message).
    pub last_green_sha: Option<String>,
    /// `git merge-base --is-ancestor main HEAD`.
    pub main_is_ancestor: bool,
    /// The bead being handed over is claimed by this worker in the ledger.
    pub bead_claimed_by_worker: bool,
    /// The bead NAMED to the gate: `--bead` on the CLI, or the id in the `bd` command on the
    /// hook path. `None` when nothing was named.
    pub bead: Option<String>,
    /// Every bead this worker holds an open claim on, from the ledger (air-xbl). This is what
    /// `air status` prints under `claims:`, and it is what a refusal names when no bead was
    /// named: the id was already computed for the digest lookup and then thrown away, so
    /// the fixing command printed a literal placeholder a worker could not run.
    pub held_beads: Vec<String>,
    /// (green, red) runs recorded at HEAD; disagreement is reported as flakiness.
    pub runs_at_head: (i64, i64),
    /// Digest check (owner ruling D, 2026-08-21): `None` when the repo configures no digest
    /// directory, or when there is no bead to declare (no bead named and no claim held,
    /// air-xbl: a batching lane that merges other workers' green work holds nothing and hands
    /// over); `Some(false)` when no digest declares a bead this worker is handing over.
    pub digest_present: Option<bool>,
    /// Where digests live (for the fixing message).
    pub digest_dir: Option<String>,
    /// Advisory mode: report what would be refused but allow (first round; decisions.md).
    pub advisory: bool,
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
}

pub fn handover_verdict(f: &GateFacts) -> Verdict {
    let mut missing = Vec::new();
    if !f.green_at_head {
        let last = f
            .last_green_sha
            .as_deref()
            .map(|s| format!(" (last green: {s})"))
            .unwrap_or_default();
        let (g, r) = f.runs_at_head;
        let (detail, fix) = if g > 0 && r > 0 {
            (
                format!(
                    "verify at HEAD {} is flaky: {g} green / {r} red; latest is red",
                    short(&f.head)
                ),
                "fix or quarantine the flaky test (file it), then: air record verify -- make verify".to_string(),
            )
        } else if let Some(tree) = f.tree_green.as_deref() {
            (
                format!(
                    "no green verify recorded at HEAD {}; {tree}",
                    short(&f.head)
                ),
                "air record verify -- make verify".to_string(),
            )
        } else {
            (
                format!("no green verify recorded at HEAD {}{last}", short(&f.head)),
                "air record verify -- make verify".to_string(),
            )
        };
        missing.push(Missing {
            check: "verify-green-at-head",
            detail,
            fix,
        });
    }
    if !f.main_is_ancestor {
        missing.push(Missing {
            check: "main-merged",
            detail: "main is not an ancestor of HEAD".to_string(),
            fix: "git merge main && air record verify -- make verify".to_string(),
        });
    }
    // air-xbl: the ids a refusal names. The named bead first; else every bead the worker
    // holds. Never a placeholder: the printed fix is the one line in the flow a worker
    // copies verbatim, and adopter's w3 was handed `air claim <bead>` while holding one.
    let beads = beads_to_name(f);
    if !f.bead_claimed_by_worker {
        let (detail, fix) = match beads.as_slice() {
            [bead] => (
                format!("{bead} is not claimed by {}", f.worker),
                format!("air claim {bead}"),
            ),
            [] => (
                format!("no bead is claimed by {}", f.worker),
                "air claim the bead you are handing over".to_string(),
            ),
            many => (
                format!("none of {} is claimed by {}", many.join(", "), f.worker),
                format!("air claim {}", many.join(" ")),
            ),
        };
        missing.push(Missing {
            check: "claim",
            detail,
            fix,
        });
    }
    if f.digest_present == Some(false) {
        let dir = f.digest_dir.as_deref().unwrap_or("docs/log.d");
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
        missing.push(Missing {
            check: "digest-present",
            detail,
            fix,
        });
    }
    let pass = missing.is_empty();
    let block = !pass && !f.advisory;
    let message = if pass {
        format!("handover ok: {} at {}", f.worker, short(&f.head))
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
        format!("handover {mode}: {}", items.join("; "))
    };
    Verdict {
        pass,
        block,
        missing,
        message,
    }
}

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// The ids a refusal may name: the bead named to the gate, else every bead the worker holds
/// (air-xbl). Empty only when neither exists.
fn beads_to_name(f: &GateFacts) -> Vec<String> {
    match &f.bead {
        Some(b) => vec![b.clone()],
        None => f.held_beads.clone(),
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
            last_green_sha: Some("f854145abcdef".into()),
            main_is_ancestor: true,
            bead_claimed_by_worker: true,
            runs_at_head: (1, 0),
            digest_present: None,
            digest_dir: None,
            bead: Some("ad-o5fi".into()),
            held_beads: vec!["ad-o5fi".into()],
            advisory: false,
        }
    }

    /// air-xbl: adopter's w3 held exactly one claim, had not written its digest, and was
    /// handed `bead: <bead>` to copy. The id was in the ledger and `air status` printed it;
    /// the refusal did not. With no bead named, the held beads are what the message names.
    #[test]
    fn a_refusal_names_the_held_bead_when_none_was_named() {
        let mut f = facts();
        f.bead = None;
        f.held_beads = vec!["ad-251z".into()];
        f.digest_present = Some(false);
        f.digest_dir = Some("docs/log.d".into());
        let v = handover_verdict(&f);
        let d = &v.missing[0];
        assert_eq!(d.check, "digest-present");
        assert!(d.detail.contains("bead: ad-251z"), "{}", d.detail);
        assert!(d.fix.contains("bead: ad-251z"), "{}", d.fix);
        assert!(!v.message.contains("<bead>"), "{}", v.message);

        // Several held: every id is named and none is invented.
        f.held_beads = vec!["fd-1".into(), "fd-2".into()];
        let v = handover_verdict(&f);
        assert!(
            v.message.contains("fd-1") && v.message.contains("fd-2"),
            "{}",
            v.message
        );
        assert!(!v.message.contains("<bead>"), "{}", v.message);

        // None held and none named: still no placeholder posing as a command.
        f.held_beads = vec![];
        f.bead_claimed_by_worker = false;
        let v = handover_verdict(&f);
        assert!(!v.message.contains("<bead>"), "{}", v.message);
        assert!(v.message.contains("holds no claim"), "{}", v.message);
    }

    /// The named bead wins over the held ones: on the hook path the id in the `bd` command is
    /// the bead being handed over, whatever else the worker holds.
    #[test]
    fn a_named_bead_is_what_the_refusal_names() {
        let mut f = facts();
        f.bead = Some("ad-named".into());
        f.held_beads = vec!["ad-other".into()];
        f.bead_claimed_by_worker = false;
        let v = handover_verdict(&f);
        assert_eq!(v.missing[0].fix, "air claim ad-named");
        assert!(!v.message.contains("ad-other"), "{}", v.message);
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
        assert_eq!(v.missing[0].fix, "air record verify -- make verify");
    }

    #[test]
    fn all_green_passes() {
        let v = handover_verdict(&facts());
        assert!(v.pass && !v.block);
        assert!(v.message.starts_with("handover ok"));
    }

    #[test]
    fn missing_verify_blocks_and_names_the_fix() {
        let mut f = facts();
        f.green_at_head = false;
        f.head = "9bb1713000".into();
        let v = handover_verdict(&f);
        assert!(!v.pass && v.block);
        assert_eq!(v.missing[0].check, "verify-green-at-head");
        assert!(v.message.contains("9bb1713"));
        assert!(v.message.contains("last green: f854145"));
        assert!(v.message.contains("air record verify"));
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
        f.bead_claimed_by_worker = claimed;
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
        vec!["fd-1".to_string(), "fd-2".to_string()]
    }

    #[test]
    fn nudges_only_a_claimless_worker_with_ready_beads_on_a_fresh_stop() {
        let r = stop_nudge("worker", false, &ids(), false).unwrap();
        assert!(r.contains("ready: fd-1, fd-2"));
        assert!(r.contains("air claim fd-1"));
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
