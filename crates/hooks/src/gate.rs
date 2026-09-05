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
    /// `git rev-parse main` at the moment the facts were read (air-5wq). Empty when unknown.
    pub main_sha: String,
    /// When main is not an ancestor: the landing that moved it past this branch, if the
    /// ledger has one (air-4up). The external cause the refusal names.
    pub main_moved: Option<MainMove>,
    /// The bead being handed over is claimed by this worker in the ledger.
    pub bead_claimed_by_worker: bool,
    pub bead: Option<String>,
    /// (green, red) runs recorded at HEAD; disagreement is reported as flakiness.
    pub runs_at_head: (i64, i64),
    /// Digest check (owner ruling D, 2026-08-21): `None` when the repo configures no digest
    /// directory (check not applicable); `Some(false)` when no digest file for this worker is
    /// newer than the claim.
    pub digest_present: Option<bool>,
    /// Where digests live (for the fixing message).
    pub digest_dir: Option<String>,
    /// Advisory mode: report what would be refused but allow (first round; decisions.md).
    pub advisory: bool,
}

/// A landing that moved main (air-4up). adopter ad-cqcr, 2026-08-30: eight refusals in one
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
        // air-4up: the cause is outside the worker's tree, so say so. "main is not an
        // ancestor of HEAD" stays in every form: adopter counts refusals by that phrase.
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
            fix: "git merge main && air record verify -- make verify".to_string(),
        });
    }
    if !f.bead_claimed_by_worker {
        let bead = f.bead.as_deref().unwrap_or("<bead>");
        missing.push(Missing {
            check: "claim",
            detail: format!("{bead} is not claimed by {}", f.worker),
            fix: format!("air claim {bead}"),
        });
    }
    if f.digest_present == Some(false) {
        let dir = f.digest_dir.as_deref().unwrap_or("docs/log.d");
        let bead = f.bead.as_deref().unwrap_or("<bead>");
        missing.push(Missing {
            check: "digest-present",
            // air-agq: the gate reads a declared `bead:` field, so the fix has to name it.
            // Saying "write a digest" was true of the old filename guess and would leave a
            // worker with a written digest and a gate that still refuses.
            detail: format!("no digest in {dir} declaring `bead: {bead}`"),
            fix: format!(
                "write {dir}/<date>-{}-{bead}.md opening with front matter:\n---\nbead: {bead}\n---",
                f.worker
            ),
        });
    }
    let pass = missing.is_empty();
    let block = !pass && !f.advisory;
    let message = if pass {
        // air-5wq: a snapshot that reads as a clearance. the adopter measured 88 refusals in
        // four days arriving within 120 s of that same worker's own `handover ok`: an answer
        // expiring before it could be used. Naming the main it was true of lets a reader see
        // at a glance whether it still applies, and the refusal (which names main too, air-4up)
        // then reads as main having moved rather than as a contradiction.
        format!(
            "handover ok: {} at {}{}",
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
        // is not the one it is about (ad-fv4z) is otherwise a true statement with no scope.
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

fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
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
            last_green_sha: Some("f854145abcdef".into()),
            main_is_ancestor: true,
            main_sha: "0a1b2c3d4e5f".into(),
            main_moved: None,
            bead_claimed_by_worker: true,
            runs_at_head: (1, 0),
            digest_present: None,
            digest_dir: None,
            bead: Some("ad-o5fi".into()),
            advisory: false,
        }
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

    /// air-4up: a refusal caused by a landing names the landing, when, and from whom, keeps
    /// the phrase adopter counts by, and keeps the fix. Without a landing to name it still
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
        assert_eq!(m.fix, "git merge main && air record verify -- make verify");

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
