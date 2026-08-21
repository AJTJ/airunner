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
    /// `verify_runs` has a green `verify` row at exactly `head` for this worker.
    pub green_at_head: bool,
    /// The most recent green sha for this worker, if any (for the message).
    pub last_green_sha: Option<String>,
    /// `git merge-base --is-ancestor main HEAD`.
    pub main_is_ancestor: bool,
    /// The bead being handed over is claimed by this worker in the ledger.
    pub bead_claimed_by_worker: bool,
    pub bead: Option<String>,
    /// Digest check (owner ruling D, 2026-08-21): `None` when the repo configures no digest
    /// directory (check not applicable); `Some(false)` when no digest file for this worker is
    /// newer than the claim.
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
        missing.push(Missing {
            check: "verify-green-at-head",
            detail: format!("no green verify recorded at HEAD {}{last}", short(&f.head)),
            fix: "air record verify -- make verify".to_string(),
        });
    }
    if !f.main_is_ancestor {
        missing.push(Missing {
            check: "main-merged",
            detail: "main is not an ancestor of HEAD".to_string(),
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
        missing.push(Missing {
            check: "digest-present",
            detail: format!("no digest by {} in {dir} newer than the claim", f.worker),
            fix: format!(
                "write {dir}/<date>-{}-<topic>.md (short architecture digest)",
                f.worker
            ),
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
            last_green_sha: Some("f854145abcdef".into()),
            main_is_ancestor: true,
            bead_claimed_by_worker: true,
            digest_present: None,
            digest_dir: None,
            bead: Some("ad-o5fi".into()),
            advisory: false,
        }
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
