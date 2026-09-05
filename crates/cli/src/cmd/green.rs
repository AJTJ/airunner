//! THE green predicate: is this commit green, and by what evidence (air-7wf).
//!
//! Every surface that asks "is HEAD green" — the hand-over gate, `air status`, the landable
//! list, `air land` — asks it here, so they cannot disagree. They did: for a week `air status`
//! read "not green" on main after every landing while `air land` had just asserted the landed
//! tree was the verified one (air-odv). Two surfaces disagreeing about one fact is the shape
//! air-y3v fixed once already.
//!
//! ## Commit, tree, and who decides
//!
//! A recorded green is looked up by **commit**, by whichever worker ran it. The worker used to
//! be part of the key; it said where a run happened, never what was verified, and the only
//! cross-worktree difference ever recorded is a test that reads where it runs (2026-08-23),
//! which roles.md rules is a defect to fix. So a batching lane's one verify now stands for
//! every worker that fast-forwards onto that commit (adopter, 2026-08-30).
//!
//! A green may ALSO be found by **tree**: the landing commit `air land` builds carries the
//! branch head's exact tree under a new sha. But **a green transfers to an identical tree only
//! if the verify is a function of the tree alone**, and that is a property of the target repo's
//! suite, which Air cannot see. adopter's `make verify` runs `git log main..HEAD` to decide
//! which beads to check (`scripts/lib/bead_citations.py:140`), so two commits over one tree
//! verify differently there and a tree-keyed gate would have handed them a false green.
//!
//! So the tree key is declared, never inferred: `"verify_key": "tree"` in `.claude/air.json`,
//! default `commit`. The failure direction is a false green on the one refusal Air makes,
//! which is why the default is the narrow one. The DISPLAY is honest either way: a tree green
//! that does not count is named as such rather than hidden behind "not green".

use std::path::Path;

use air_ledger::Ledger;
use air_ledger::verify::{GreenAt, Kind};
use serde::Serialize;

use crate::git;

/// How a repo's recorded greens are matched to a commit. Read from `.claude/air.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Key {
    /// A green at this exact commit. The default, and the safe one.
    Commit,
    /// A green at this commit OR at any commit with the identical tree. Declared by the repo,
    /// which is asserting that its verify reads nothing but the tree.
    Tree,
}

impl Key {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "commit" => Some(Key::Commit),
            "tree" => Some(Key::Tree),
            _ => None,
        }
    }
}

/// The repo's declared key: `verify_key` in `.claude/air.json`, `commit` when absent or
/// unrecognised. An unrecognised value is the narrow key, not an error: the gate fails toward
/// refusing, never toward permitting.
pub fn key_for(repo: &Path) -> Key {
    super::handover::air_json(repo)
        .and_then(|j| j.get("verify_key")?.as_str().and_then(Key::parse))
        .unwrap_or(Key::Commit)
}

/// What the ledger holds for a commit, and the policy it is read under.
#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub at: Option<GreenAt>,
    pub key: Key,
}

impl Evidence {
    /// Does this commit count as green under the repo's key?
    pub fn holds(&self) -> bool {
        match &self.at {
            Some(GreenAt::Commit(_)) => true,
            Some(GreenAt::Tree(_)) => self.key == Key::Tree,
            None => false,
        }
    }

    /// The one-word answer `air status` prints.
    pub fn word(&self) -> &'static str {
        if self.holds() { "green" } else { "not green" }
    }

    /// What is behind the word, when there is anything to say: which commit the green was
    /// recorded at and by whom, when it is not this one. Empty for a plain commit green or a
    /// plain absence, so the ordinary line does not grow.
    pub fn detail(&self) -> Option<String> {
        match &self.at {
            Some(GreenAt::Tree(r)) if self.key == Key::Tree => Some(format!(
                "same tree as {} verified by {}",
                short(&r.sha),
                r.worker
            )),
            Some(GreenAt::Tree(r)) => Some(format!(
                "this exact tree is green at {} by {}, but this repo keys green by commit \
                 (`verify_key` in .claude/air.json)",
                short(&r.sha),
                r.worker
            )),
            _ => None,
        }
    }

    /// `word`, then `detail` in parentheses when there is one.
    pub fn line(&self) -> String {
        match self.detail() {
            Some(d) => format!("{} ({d})", self.word()),
            None => self.word().to_string(),
        }
    }
}

/// The evidence for `sha` under the repo's declared key. One ledger read on the ordinary
/// path; the tree is resolved with `git rev-parse` only when the commit itself has no run,
/// which keeps the hook path at one query when the worker has done what it should.
pub fn at(ledger: &Ledger, repo: &Path, sha: &str, kind: Kind) -> Result<Evidence, String> {
    at_under(ledger, repo, sha, kind, key_for(repo))
}

/// [`at`] with the key supplied, for probes that have no `.claude/air.json` to read.
pub fn at_under(
    ledger: &Ledger,
    repo: &Path,
    sha: &str,
    kind: Kind,
    key: Key,
) -> Result<Evidence, String> {
    if let Some(run) = ledger
        .latest_run_at_commit(sha, kind)
        .map_err(|e| e.to_string())?
    {
        return Ok(Evidence {
            at: run.is_green().then_some(GreenAt::Commit(run)),
            key,
        });
    }
    // Any checkout of the repo can resolve the tree: worktrees share one object store.
    let tree = tree_of(repo, sha).ok();
    let at = ledger
        .green_at(sha, tree.as_deref(), kind)
        .map_err(|e| e.to_string())?;
    Ok(Evidence { at, key })
}

/// The tree id of a commit.
pub fn tree_of(repo: &Path, sha: &str) -> git::Result<String> {
    git::run(repo, &["rev-parse", &format!("{sha}^{{tree}}")])
}

fn short(sha: &str) -> &str {
    sha.get(..8).unwrap_or(sha)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use air_ledger::verify::{VerifyRun, new_id};

    use super::*;

    fn run(sha: &str, tree: &str, exit: i32) -> VerifyRun {
        VerifyRun {
            id: new_id(),
            worker: "w1".into(),
            sha: sha.into(),
            kind: Kind::Verify,
            exit_code: exit,
            trigger: "test".into(),
            failing_step: None,
            started_at: "t".into(),
            finished_at: "t".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: Some(tree.into()),
        }
    }

    fn evidence(at: Option<GreenAt>, key: Key) -> Evidence {
        Evidence { at, key }
    }

    #[test]
    fn a_tree_green_counts_only_under_the_tree_key() {
        let tree = Some(GreenAt::Tree(run("branch", "T", 0)));
        assert!(!evidence(tree.clone(), Key::Commit).holds());
        assert!(evidence(tree.clone(), Key::Tree).holds());
        // A commit green counts under either.
        let commit = Some(GreenAt::Commit(run("branch", "T", 0)));
        assert!(evidence(commit.clone(), Key::Commit).holds());
        assert!(evidence(commit, Key::Tree).holds());
        assert!(!evidence(None, Key::Tree).holds());
    }

    /// The display is honest under both keys: a tree green that does not count is named,
    /// not hidden behind "not green".
    #[test]
    fn the_line_names_the_tree_green_either_way() {
        let tree = Some(GreenAt::Tree(run("branchhead1", "T", 0)));
        let counted = evidence(tree.clone(), Key::Tree).line();
        assert!(counted.starts_with("green ("), "{counted}");
        assert!(counted.contains("branchhe"), "{counted}");
        let not = evidence(tree, Key::Commit).line();
        assert!(not.starts_with("not green ("), "{not}");
        assert!(not.contains("verify_key"), "{not}");
        assert_eq!(
            evidence(Some(GreenAt::Commit(run("a", "T", 0))), Key::Commit).line(),
            "green"
        );
        assert_eq!(evidence(None, Key::Commit).line(), "not green");
    }

    #[test]
    fn the_key_defaults_to_commit_and_an_unknown_value_stays_narrow() {
        assert_eq!(Key::parse("tree"), Some(Key::Tree));
        assert_eq!(Key::parse("commit"), Some(Key::Commit));
        assert_eq!(Key::parse("yes"), None);
        // No .claude/air.json anywhere under a temp dir that is not a repo.
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(key_for(dir.path()), Key::Commit);
    }
}
