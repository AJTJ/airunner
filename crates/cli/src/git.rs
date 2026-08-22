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
    let child = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(GitError::Spawn)?;
    let (status, stdout, stderr) = match wait_drained(child, TIMEOUT).map_err(GitError::Spawn)? {
        Some(x) => x,
        None => return Err(GitError::Timeout(TIMEOUT)),
    };
    if !status.success() {
        return Err(GitError::Failed {
            args: args.iter().map(|s| s.to_string()).collect(),
            code: status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&stdout).trim_end().to_string())
}

/// (exit status, stdout, stderr) of a finished child.
pub(crate) type Drained = (std::process::ExitStatus, Vec<u8>, Vec<u8>);

/// Wait for a child with a timeout while draining its pipes on threads, so a child that
/// writes more than the pipe buffer (64 KB) cannot deadlock against us and be mistaken for
/// a hang. Kills and reaps on timeout. Returns (status, stdout, stderr).
pub(crate) fn wait_drained(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> std::io::Result<Option<Drained>> {
    use std::io::Read;
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let err_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let status = match child.wait_timeout(timeout)? {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            // Pipes close when the child dies; the drain threads finish.
            let _ = out_t.join();
            let _ = err_t.join();
            return Ok(None);
        }
    };
    let stdout = out_t.join().unwrap_or_default();
    let stderr = err_t.join().unwrap_or_default();
    Ok(Some((status, stdout, stderr)))
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

/// Committer time (RFC 3339) of the branch point of HEAD from `base`.
pub fn branch_point_time(cwd: &Path, base: &str) -> Result<String> {
    let mb = run(cwd, &["merge-base", base, "HEAD"])?;
    run(cwd, &["log", "-1", "--format=%cI", &mb])
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

/// Tracked files with uncommitted changes, repo-relative. Untracked files are excluded on
/// purpose: `air land` refuses on this because its rollback is `git reset --hard`, which
/// restores tracked files only and leaves untracked ones alone (adopter `land.sh:504-514`).
pub fn dirty_tracked(cwd: &Path) -> Result<Vec<String>> {
    let out = run(cwd, &["status", "--porcelain=v1", "--untracked-files=no"])?;
    Ok(out
        .lines()
        .filter_map(|l| l.get(3..))
        .map(|p| p.rsplit(" -> ").next().unwrap_or(p).to_string())
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod drain_tests {
    use super::wait_drained;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    #[test]
    fn large_output_is_drained_not_mistaken_for_a_hang() {
        let child = Command::new("sh")
            .args([
                "-c",
                "head -c 1048576 /dev/zero | tr '\\0' 'x'; echo err >&2",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (status, out, err) = wait_drained(child, Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(status.success());
        assert_eq!(out.len(), 1_048_576);
        assert_eq!(String::from_utf8_lossy(&err).trim(), "err");
    }

    #[test]
    fn a_real_hang_is_killed_and_reported() {
        let child = Command::new("sh")
            .args(["-c", "sleep 5"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let t = std::time::Instant::now();
        assert!(
            wait_drained(child, Duration::from_millis(100))
                .unwrap()
                .is_none()
        );
        assert!(t.elapsed() < Duration::from_secs(2));
    }
}
