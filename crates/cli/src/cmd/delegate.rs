//! A pinned repo runs its pin, whichever `air` a shell found (air-qyrm).
//!
//! The 0.4.0 live trial (2026-09-25) pinned a copy at `<main>/.air/bin/air` and put that
//! directory first on every launched session's PATH. Every session's bare `air` still ran the
//! 0.2.19 on PATH: Claude Code's Bash tool starts a shell that sources the user's profile, and
//! the profile re-orders PATH. So PATH order is not something Air can hold. What it can hold is
//! this: before any command runs, an `air` that finds itself in a pinned repo and is not the pin
//! re-executes the pin with the same arguments and environment. That works whatever PATH order a
//! shell produces, as long as the `air` a shell finds is one that carries this file.
//!
//! The repo is found from files alone (`.git`, and a linked worktree's `gitdir` and
//! `commondir`), not by spawning git: this runs on every invocation, the hook included, and the
//! answer is a handful of stats. The hook never delegates in practice, because a pinned repo's
//! hooks already name the pin by absolute path.
//!
//! Loops: the pin compares its own canonical path to the pin's and finds them equal. In case
//! they ever disagree (a pin that is a hard link, a filesystem that canonicalizes oddly), the
//! delegating process marks the environment with its own pid, which `exec` keeps; the process
//! that finds its own pid there never delegates. A child it starts has another pid, so a
//! session launched from a delegated `air` still delegates its own bare `air`.
//!
//! `air install` never delegates: it copies the running binary into the pin, so re-pinning a
//! repo with a newer candidate must run the candidate, not the pin it replaces.
//!
//! Removal: when the `air` every pinned repo's shells find is the pin (the harness keeps the
//! PATH Air gives a session), or when pinning goes (`install::pin_path`'s own condition).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// `<pid>:<the binary that delegated>`, set on the pin's environment.
pub const MARKER: &str = "AIR_DELEGATED";

/// Pure: the `--repo` value and the subcommand in an argument list (without argv\[0\]). Stops at
/// `--`, so `air record verify -- make --repo x` does not read the recorded command.
pub fn scan(args: &[OsString]) -> (Option<PathBuf>, Option<OsString>) {
    let (mut repo, mut sub) = (None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            break;
        }
        if a == "--repo" {
            repo = it.next().map(PathBuf::from);
            continue;
        }
        if let Some(v) = a.to_str().and_then(|s| s.strip_prefix("--repo=")) {
            repo = Some(PathBuf::from(v));
            continue;
        }
        if sub.is_none() && !a.to_string_lossy().starts_with('-') {
            sub = Some(a.clone());
        }
    }
    (repo, sub)
}

/// The main checkout of the repository containing `start`, from files alone: the first
/// ancestor with a `.git`. A `.git` directory is the main checkout; a `.git` file is a linked
/// worktree, whose `gitdir` holds a `commondir` naming `<main>/.git`. None outside a repo, and
/// for a `.git` file with no `commondir` (a submodule), which has no pin of its own.
pub fn main_checkout(start: &Path) -> Option<PathBuf> {
    for d in start.ancestors() {
        let dot = d.join(".git");
        let Ok(meta) = std::fs::metadata(&dot) else {
            continue;
        };
        if meta.is_dir() {
            return Some(d.to_path_buf());
        }
        let text = std::fs::read_to_string(&dot).ok()?;
        let gitdir = d.join(text.strip_prefix("gitdir:")?.trim());
        let common = std::fs::read_to_string(gitdir.join("commondir")).ok()?;
        let common = gitdir.join(common.trim()).canonicalize().ok()?;
        return common.parent().map(Path::to_path_buf);
    }
    None
}

/// Whether `marker` says this very process was delegated to (same pid).
fn marked_for(marker: Option<&OsStr>, pid: u32) -> bool {
    marker
        .and_then(OsStr::to_str)
        .and_then(|m| m.split_once(':'))
        .is_some_and(|(p, _)| p == pid.to_string())
}

/// The pin to hand this invocation to, or None to run here.
pub fn target(
    args: &[OsString],
    cwd: &Path,
    exe: &Path,
    marker: Option<&OsStr>,
    pid: u32,
) -> Option<PathBuf> {
    if marked_for(marker, pid) {
        return None;
    }
    let (repo, sub) = scan(args);
    if sub.as_deref() == Some(OsStr::new("install")) {
        return None;
    }
    let repo = cwd.join(repo.unwrap_or_default());
    let pin = super::install::pin_path(&main_checkout(&repo)?.join(".air"));
    if !pin.is_file() {
        return None;
    }
    let pin = pin.canonicalize().ok()?;
    (exe.canonicalize().ok()? != pin).then_some(pin)
}

/// Re-execute the pin if this repo has one and this is not it. Returns only when this binary
/// should run the command itself; a failed exec is said on stderr and runs here (fail open).
pub fn maybe_exec() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let (Ok(cwd), Ok(exe)) = (std::env::current_dir(), std::env::current_exe()) else {
        return;
    };
    let marker = std::env::var_os(MARKER);
    let Some(pin) = target(&args, &cwd, &exe, marker.as_deref(), std::process::id()) else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(&pin)
            .args(&args)
            .env(MARKER, format!("{}:{}", std::process::id(), exe.display()))
            .exec();
        eprintln!(
            "air: this repo is pinned to {} but running it failed ({err}); running {} instead",
            pin.display(),
            exe.display()
        );
    }
}

/// The binary that delegated to this process, when one did.
pub fn delegated_from() -> Option<String> {
    let m = std::env::var_os(MARKER)?;
    if !marked_for(Some(&m), std::process::id()) {
        return None;
    }
    m.to_str()?
        .split_once(':')
        .map(|(_, from)| from.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<OsString> {
        a.iter().map(OsString::from).collect()
    }

    #[test]
    fn scan_reads_repo_and_subcommand_and_stops_at_double_dash() {
        let (r, s) = scan(&v(&["--json", "--repo", "/x", "status"]));
        assert_eq!(r, Some(PathBuf::from("/x")));
        assert_eq!(s, Some(OsString::from("status")));
        let (r, s) = scan(&v(&["record", "verify", "--", "make", "--repo", "/y"]));
        assert_eq!(r, None);
        assert_eq!(s, Some(OsString::from("record")));
        let (r, _) = scan(&v(&["--repo=/z", "--version"]));
        assert_eq!(r, Some(PathBuf::from("/z")));
    }

    #[test]
    fn a_process_marked_with_its_own_pid_never_delegates() {
        assert!(marked_for(Some(OsStr::new("42:/usr/bin/air")), 42));
        assert!(!marked_for(Some(OsStr::new("41:/usr/bin/air")), 42));
        assert!(!marked_for(None, 42));
    }
}
