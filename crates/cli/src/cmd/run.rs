//! `air run -- <command…>`: run a command on the calling session's target, or locally.
//!
//! A target is a machine the session owns, named in `.claude/air.json` under `"targets"`,
//! keyed by session name:
//!
//! ```json
//! {"targets": {"worker-1": {"kind": "ssh", "host": "box", "user": "me", "path": "~/air/proj"}}}
//! ```
//!
//! A session with no target runs the command here, in the worktree, with the same exit code
//! and nothing added to its output. With an `ssh` target Air initialises a repository at
//! `path` on the host the first time, pushes HEAD to `refs/air/<session>` there (force: the
//! ref is the session's own), checks that commit out detached, and runs the command in `path`
//! over ssh with its output streamed back as it arrives. The exit code is the command's.
//!
//! The push goes to the configured target only, never to a remote named in `.git/config`, so
//! the rule that nothing leaves the machine without the owner's word is untouched. `ssh` is
//! resolved on `PATH`, which is also how a test stands in for it.
//!
//! Recorded failure: an adopter's workers ran Linux-only and heavy commands on the Mac slowly
//! or not at all, or reached a pod by hand outside Air, so no record said which machine a
//! number came from (adopter, 2026-10-29/30; epic air-opq). This child is the walking
//! skeleton: no ledger row, no result paths back. Removed when the harness or the repo's own
//! tooling runs a command on a named remote and records the machine and the commit.

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use crate::git;

/// One `ssh` target from `"targets"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub host: String,
    pub user: Option<String>,
    /// The repository directory on the host. A leading `~` is the remote shell's to expand.
    pub path: String,
}

impl Target {
    /// `user@host`, or `host`.
    fn dest(&self) -> String {
        match &self.user {
            Some(u) => format!("{u}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

/// The target for `session`, pure over the parsed config: None when `targets` or the session
/// is absent, Err naming the fault when the entry is there and unusable.
pub fn target_for(cfg: &serde_json::Value, session: &str) -> Result<Option<Target>, String> {
    let Some(entry) = cfg.get("targets").and_then(|t| t.get(session)) else {
        return Ok(None);
    };
    let field = |k: &str| entry.get(k).and_then(|v| v.as_str()).map(str::to_string);
    match field("kind").as_deref() {
        Some("ssh") => {}
        Some(other) => {
            return Err(format!(
                "targets.{session}.kind is {other:?}; this air knows \"ssh\""
            ));
        }
        None => return Err(format!("targets.{session} has no \"kind\"")),
    }
    let host = field("host").ok_or_else(|| format!("targets.{session} has no \"host\""))?;
    let path = field("path").ok_or_else(|| format!("targets.{session} has no \"path\""))?;
    Ok(Some(Target {
        host,
        user: field("user"),
        path,
    }))
}

pub fn run(repo: &Path, command: &[String]) -> i32 {
    let Some((prog, rest)) = command.split_first() else {
        eprintln!("air run: no command given, e.g. `air run -- make test`");
        return 2;
    };
    let session = match air_ledger::paths::worker_name_for(repo) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("air run: {e}");
            return 1;
        }
    };
    let target = match super::handover::air_json(repo) {
        Some(cfg) => target_for(&cfg, &session),
        None => Ok(None),
    };
    match target {
        Ok(None) => local(repo, prog, rest),
        Ok(Some(t)) => remote(repo, &session, &t, command),
        Err(why) => {
            eprintln!("air run: .claude/air.json: {why}");
            2
        }
    }
}

/// No target: the command, here, with our stdio.
fn local(repo: &Path, prog: &str, args: &[String]) -> i32 {
    match Command::new(prog).args(args).current_dir(repo).status() {
        Ok(s) => code(s),
        Err(e) => {
            eprintln!("air run: {prog}: {e}");
            127
        }
    }
}

fn remote(repo: &Path, session: &str, t: &Target, command: &[String]) -> i32 {
    let sha = match git::head(repo) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("air run: {e}");
            return 1;
        }
    };
    let dest = t.dest();
    let path = sh_quote(&t.path);
    let short = sha.get(..8).unwrap_or(&sha);
    eprintln!("air run: {session} at {short} on {dest}:{}", t.path);
    // Once: a repository at the path. Idempotent, so every run may ask.
    let init = format!("test -d {path}/.git || {{ mkdir -p {path} && git init -q {path}; }}");
    if let Some(code) = step("init", ssh(&dest, &init)) {
        return code;
    }
    // Force, because the ref is this session's own and an amend or a rebase moves it back.
    let push = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["push", "-q", "--force"])
        .arg(format!("{dest}:{}", t.path))
        .arg(format!("{sha}:refs/air/{session}"))
        .stdin(Stdio::null())
        .status();
    if let Some(code) = step("push", push) {
        return code;
    }
    let checkout = format!("cd {path} && git checkout -q --detach {sha}");
    if let Some(code) = step("checkout", ssh(&dest, &checkout)) {
        return code;
    }
    let cmd = command
        .iter()
        .map(|a| sh_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    match ssh(&dest, &format!("cd {path} && {cmd}")) {
        Ok(s) => code(s),
        Err(e) => {
            eprintln!("air run: ssh: {e}");
            127
        }
    }
}

/// `ssh <dest> <script>`, stdout and stderr ours so output streams as it arrives.
fn ssh(dest: &str, script: &str) -> std::io::Result<ExitStatus> {
    Command::new("ssh")
        .arg(dest)
        .arg(script)
        .stdin(Stdio::null())
        .status()
}

/// A preparatory step's failure as an exit code, naming the step; None when it succeeded.
fn step(what: &str, result: std::io::Result<ExitStatus>) -> Option<i32> {
    match result {
        Ok(s) if s.success() => None,
        Ok(s) => {
            eprintln!("air run: {what} failed (exit {})", code(s));
            Some(code(s))
        }
        Err(e) => {
            eprintln!("air run: {what}: {e}");
            Some(127)
        }
    }
}

/// The child's exit code; a signal reads as a shell reports it, 128 plus the signal.
fn code(s: ExitStatus) -> i32 {
    if let Some(c) = s.code() {
        return c;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = s.signal() {
            return 128i32.saturating_add(sig);
        }
    }
    1
}

/// Quote one word for a POSIX shell. A word of safe characters stays bare, so a leading `~`
/// still expands on the remote.
pub fn sh_quote(s: &str) -> String {
    let safe = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./~:=+,@%".contains(c));
    if safe {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn target_absent_or_present() {
        let cfg = serde_json::json!({"targets": {"worker-1": {"kind": "ssh", "host": "box", "path": "~/air/p"}}});
        assert_eq!(target_for(&cfg, "worker-2").unwrap(), None);
        assert_eq!(
            target_for(&serde_json::json!({}), "worker-1").unwrap(),
            None
        );
        let t = target_for(&cfg, "worker-1").unwrap().unwrap();
        assert_eq!(t.dest(), "box");
        assert_eq!(t.path, "~/air/p");
    }

    #[test]
    fn target_faults_are_named() {
        let cfg =
            serde_json::json!({"targets": {"lane": {"kind": "exec", "host": "x", "path": "p"}}});
        assert!(target_for(&cfg, "lane").unwrap_err().contains("\"exec\""));
        let cfg = serde_json::json!({"targets": {"lane": {"kind": "ssh", "host": "x"}}});
        assert!(target_for(&cfg, "lane").unwrap_err().contains("\"path\""));
    }

    #[test]
    fn quoting() {
        assert_eq!(sh_quote("~/air/p"), "~/air/p");
        assert_eq!(sh_quote("cargo"), "cargo");
        assert_eq!(sh_quote("a b"), "'a b'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(sh_quote(""), "''");
    }
}
