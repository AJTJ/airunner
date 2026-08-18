//! Thin, timeout-bounded `git` calls. Shelling to `git` costs ~10 ms per call on this Mac
//! (tick 0315); we call it only from CLI paths and once per hook at most.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git not runnable: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("git timed out after {0:?}")]
    Timeout(Duration),
    #[error("git {args:?} exited {code}: {stderr}")]
    Failed {
        args: Vec<String>,
        code: i32,
        stderr: String,
    },
}

pub type Result<T> = std::result::Result<T, GitError>;

const TIMEOUT: Duration = Duration::from_millis(1500);

pub fn run(cwd: &Path, args: &[&str]) -> Result<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(GitError::Spawn)?;
    let status = match child.wait_timeout(TIMEOUT) {
        Ok(Some(s)) => s,
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(GitError::Timeout(TIMEOUT));
        }
        Err(e) => return Err(GitError::Spawn(e)),
    };
    let out = child.wait_with_output().map_err(GitError::Spawn)?;
    if !status.success() {
        return Err(GitError::Failed {
            args: args.iter().map(|s| s.to_string()).collect(),
            code: status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

pub fn head(cwd: &Path) -> Result<String> {
    run(cwd, &["rev-parse", "HEAD"])
}

pub fn toplevel(cwd: &Path) -> Result<PathBuf> {
    Ok(PathBuf::from(run(
        cwd,
        &["rev-parse", "--path-format=absolute", "--show-toplevel"],
    )?))
}

/// `git merge-base --is-ancestor <anc> <desc>` — exit 0 yes, 1 no, other = error.
pub fn is_ancestor(cwd: &Path, anc: &str, desc: &str) -> Result<bool> {
    match run(cwd, &["merge-base", "--is-ancestor", anc, desc]) {
        Ok(_) => Ok(true),
        Err(GitError::Failed { code: 1, .. }) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Files with uncommitted changes (staged or not) plus untracked, repo-relative.
pub fn dirty_files(cwd: &Path) -> Result<Vec<String>> {
    let out = run(cwd, &["status", "--porcelain=v1", "--untracked-files=all"])?;
    Ok(out
        .lines()
        .filter_map(|l| l.get(3..))
        .map(|p| {
            // renames print "old -> new"; keep the new path
            p.rsplit(" -> ").next().unwrap_or(p).to_string()
        })
        .collect())
}

/// Files changed on this branch relative to `base` (committed divergence).
pub fn changed_since(cwd: &Path, base: &str) -> Result<Vec<String>> {
    let out = run(cwd, &["diff", "--name-only", &format!("{base}...HEAD")])?;
    Ok(out.lines().map(str::to_string).collect())
}

/// All worktrees of this repository: (path, branch or None).
pub fn worktrees(cwd: &Path) -> Result<Vec<(PathBuf, Option<String>)>> {
    let out = run(cwd, &["worktree", "list", "--porcelain"])?;
    let mut res = Vec::new();
    let mut cur: Option<(PathBuf, Option<String>)> = None;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(c) = cur.take() {
                res.push(c);
            }
            cur = Some((PathBuf::from(p), None));
        } else if let Some(b) = line.strip_prefix("branch refs/heads/")
            && let Some(c) = cur.as_mut()
        {
            c.1 = Some(b.to_string());
        }
    }
    if let Some(c) = cur.take() {
        res.push(c);
    }
    Ok(res)
}
