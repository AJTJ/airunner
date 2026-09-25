//! Air creates, fills and removes its worker worktrees (air-fdz; owner, 2026-08-30: "move
//! back to Air managing worktrees, and audit rather than implement blindly").
//!
//! What the audit found, and what this module does about each piece:
//!
//! - **Creation** was the one piece delegated to `claude --worktree`. It is
//!   `git worktree add -b worktree-<name> <main>/.claude/worktrees/<name>` here, the same
//!   directory and branch names the harness uses, so `air status`, `worker_name_for`, the
//!   ledger and every existing worktree keep reading as before.
//! - **`.worktreeinclude`.** `--worktree` copies gitignored files matching the repo's
//!   `.worktreeinclude` (gitignore syntax) into the new worktree. The adopter's workers do not
//!   function without it: `backend/.env`, `app/.env`, and `backend/keys/*.pem`, which
//!   `authn.rs:233` reads with `include_str!` at COMPILE TIME, so a naive `git worktree add`
//!   gives a fleet whose backend test crate does not build, with an error that does not say
//!   why (their own note, an agent lost time to it on 2026-08-13). [`copy_included`] does the
//!   same copy, with git's own glob engine: each pattern becomes a `:(glob)` pathspec and
//!   `git ls-files --others --ignored --exclude-standard` lists exactly the ignored, untracked
//!   files that match. Symlinks are skipped and a destination outside the worktree is refused,
//!   as the harness does (its messages: "Skipping symlink in .worktreeinclude", "destination
//!   escapes worktree via committed symlink"). Negated patterns are not supported and are
//!   reported rather than silently dropped.
//! - **Isolation stays the harness's.** The worktree is still handed to claude by name
//!   (`--worktree <name>` adopts an existing `.claude/worktrees/<name>`, which is how every
//!   relaunch has worked), because the refusal of out-of-worktree git and of edits to the main
//!   checkout is keyed on the harness's own record of the session's worktree
//!   (`La()?.worktreePath` in 2.1.261) and exists only with the flag. Dropping the flag would
//!   remove a working enforcement that roles.md promises, for a coupling argument with no
//!   incident behind it (do-less). What changes is who creates and who removes.
//! - **WorktreeCreate hook.** The adopter's file says configuring one "replaces git's worktree
//!   logic entirely and this file stops being processed". Checked against 2.1.261's messages
//!   rather than exercised: the harness uses the hook's returned path in place of its own git
//!   logic ("Cannot create agent worktree: not in a git repository and no WorktreeCreate hooks
//!   are configured … to use worktree isolation with other VCS systems"; "hook succeeded but
//!   returned no worktree path"), and the `.worktreeinclude` copy lives in the git path. The
//!   claim is consistent with the binary; Air configures no such hook.
//! - **Removal** is Air's too, and never silent: a worktree with uncommitted work, a live tmux
//!   session, or a harness lock is refused with what is holding it and the command that
//!   would force it. The branch is kept; a branch is history, a directory is not.
//!
//! Removal condition for the whole module: the day the harness exposes worktree creation
//! with `.worktreeinclude` as a command Air can call without a session (a `claude worktree
//! add`), or the owner reverses the 2026-08-30 ruling.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// The main checkout for `repo` (the ledger's directory, minus `.air`).
pub fn main_checkout(repo: &Path) -> PathBuf {
    air_ledger::paths::air_dir_for(repo)
        .ok()
        .and_then(|d| d.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| repo.to_path_buf())
}

/// Where a worker's worktree lives: the harness's own layout, so nothing that reads
/// `.claude/worktrees/<name>` (the ledger's `worker_name_for`, `air status`) changes.
pub fn dir_for(main: &Path, name: &str) -> PathBuf {
    main.join(".claude").join("worktrees").join(name)
}

/// The branch a worker's worktree is on: `worktree-<name>`, as the harness names it.
pub fn branch_for(name: &str) -> String {
    format!("worktree-{name}")
}

/// git with a longer budget than `git.rs`'s 1.5 s: `worktree add` checks out a whole tree and
/// the include listing walks ignored directories (`node_modules`), both of which can take
/// seconds in a real repo.
fn git(cwd: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let (code, stdout, stderr) = git_status(cwd, args, timeout)?;
    if code != 0 {
        return Err(format!(
            "git {} exited {code}: {}",
            args.join(" "),
            stderr.trim()
        ));
    }
    Ok(stdout)
}

/// The same budget with the exit code handed back rather than turned into an error:
/// `air batch cut` asks `git merge` and `git merge-tree`, whose exit 1 is an answer (a
/// conflict), not a failure, and whose stdout is needed on exactly that path.
pub(crate) fn git_status(
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<(i32, String, String), String> {
    let t0 = std::time::Instant::now();
    let child = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git: {e}"))?;
    let drained = crate::git::wait_drained(child, timeout).map_err(|e| format!("git: {e}"));
    air_ledger::budgets::record(
        air_ledger::budgets::GIT_WORKTREE,
        t0.elapsed(),
        timeout,
        matches!(&drained, Ok(None)),
    );
    let (status, stdout, stderr) = match drained? {
        Some(x) => x,
        None => {
            return Err(format!(
                "git {} timed out after {timeout:?}",
                args.join(" ")
            ));
        }
    };
    Ok((
        status.code().unwrap_or(-1),
        String::from_utf8_lossy(&stdout).to_string(),
        String::from_utf8_lossy(&stderr).to_string(),
    ))
}

/// **Fail direction: CLOSED.** `air worker --create` and worktree removal are CLI commands the
/// coordinator watches; a hit errors and names the git command. Nothing decides on it and no
/// hook path reaches it, so this is a convenience budget and may fail open in the sense that
/// matters (it never permits anything).
///
/// Not derived: a `worktree add` on a large repo is minutes-scale work whose distribution has
/// nothing to do with the one-shot `git` calls elsewhere. Moved by `air audit`'s
/// `git-worktree` row — a p99 anywhere near 120 s means a repo where creating a worktree needs
/// its own budget rather than a bigger constant.
pub(crate) const GIT_BUDGET: Duration = Duration::from_secs(120);

/// Pure: one `.worktreeinclude` line (gitignore syntax) as a git pathspec with `:(glob)`
/// magic, so `git ls-files` does the matching with its own engine. `None` for a blank line, a
/// comment, or a negation (`!`), which this does not support.
///
/// gitignore reads a pattern with no slash as "at any depth" and one with a slash as anchored
/// to the root; `:(glob)` reads `**/` as "at any depth" and everything else as anchored, so:
/// `.env` → `**/.env`; `backend/.env` → `backend/.env`; `/keys` → `keys`; `keys/` (a
/// directory) → `keys/**`, at any depth when it had no other slash.
pub fn pathspec_for(line: &str) -> Option<String> {
    let p = line.trim_end();
    let p = p.trim_start();
    if p.is_empty() || p.starts_with('#') || p.starts_with('!') {
        return None;
    }
    let (dir, p) = match p.strip_suffix('/') {
        Some(rest) => (true, rest),
        None => (false, p),
    };
    let anchored = p.contains('/');
    let p = p.strip_prefix('/').unwrap_or(p);
    let mut spec = String::from(":(glob)");
    if !anchored {
        spec.push_str("**/");
    }
    spec.push_str(p);
    if dir {
        spec.push_str("/**");
    }
    Some(spec)
}

/// The pathspecs from a `.worktreeinclude` text and the negated lines it could not honour.
pub fn include_specs(text: &str) -> (Vec<String>, Vec<String>) {
    let mut specs = Vec::new();
    let mut unsupported = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('!') {
            unsupported.push(t.to_string());
        } else if let Some(s) = pathspec_for(line) {
            specs.push(s);
        }
    }
    (specs, unsupported)
}

/// The ignored, untracked files under `main` that `.worktreeinclude` names, repo-relative.
/// Empty when there is no such file.
pub fn included_files(main: &Path) -> Result<(Vec<PathBuf>, Vec<String>), String> {
    let text = match std::fs::read_to_string(main.join(".worktreeinclude")) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Vec::new()));
        }
        Err(e) => return Err(format!(".worktreeinclude: {e}")),
    };
    let (specs, unsupported) = include_specs(&text);
    if specs.is_empty() {
        return Ok((Vec::new(), unsupported));
    }
    let mut args: Vec<&str> = vec![
        "ls-files",
        "-z",
        "--others",
        "--ignored",
        "--exclude-standard",
        "--full-name",
        "--",
    ];
    args.extend(specs.iter().map(String::as_str));
    let out = git(main, &args, GIT_BUDGET)?;
    let files = out
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    Ok((files, unsupported))
}

/// What one copy did, for the launch line and the probe.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Copied {
    pub copied: Vec<PathBuf>,
    /// Symlinks, and destinations that would escape the worktree: named, never copied.
    pub skipped: Vec<String>,
    pub unsupported: Vec<String>,
}

/// Copy `.worktreeinclude`'s files from `main` into `wt`. Parents are created; a file is
/// copied with its mode (`fs::copy`); an existing destination is overwritten, since a relaunch
/// wants the current `.env`, not the one from the day the lane was cut.
pub fn copy_included(main: &Path, wt: &Path) -> Result<Copied, String> {
    let (files, unsupported) = included_files(main)?;
    let mut done = Copied {
        unsupported,
        ..Default::default()
    };
    let wt_canon = wt
        .canonicalize()
        .map_err(|e| format!("{}: {e}", wt.display()))?;
    for rel in files {
        let src = main.join(&rel);
        let meta = match std::fs::symlink_metadata(&src) {
            Ok(m) => m,
            Err(e) => {
                done.skipped.push(format!("{}: {e}", rel.display()));
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            done.skipped.push(format!("{}: symlink", rel.display()));
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let dst = wt.join(&rel);
        let Some(parent) = dst.parent() else { continue };
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        // A committed symlink on the destination path could point outside the worktree.
        let parent_canon = parent
            .canonicalize()
            .map_err(|e| format!("{}: {e}", parent.display()))?;
        if !parent_canon.starts_with(&wt_canon) {
            done.skipped.push(format!(
                "{}: destination escapes the worktree via a symlink",
                rel.display()
            ));
            continue;
        }
        std::fs::copy(&src, &dst).map_err(|e| format!("{}: {e}", rel.display()))?;
        done.copied.push(rel);
    }
    Ok(done)
}

/// What `ensure` found or made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    pub path: PathBuf,
    pub branch: String,
    /// True when the worktree was already there: a relaunch. The include copy runs either way.
    pub existed: bool,
    pub copied: Copied,
}

/// Is `path` a worktree of this repository?
fn is_worktree(main: &Path, path: &Path) -> bool {
    crate::git::worktrees(main)
        .unwrap_or_default()
        .iter()
        .any(|(p, _)| p == path || p.canonicalize().ok() == path.canonicalize().ok())
}

/// Make `<main>/.claude/worktrees/<name>` exist as a worktree on `worktree-<name>`, and fill
/// it from `.worktreeinclude`. A branch left behind by a removed worktree is reused rather
/// than refused, since it is the lane's history. Requires a commit to branch from.
pub fn ensure(main: &Path, name: &str) -> Result<Created, String> {
    let path = dir_for(main, name);
    let branch = branch_for(name);
    let existed = path.is_dir() && is_worktree(main, &path);
    if path.exists() && !existed {
        return Err(format!(
            "{} exists but is not a worktree of this repository; move it aside or pick \
             another name",
            path.display()
        ));
    }
    if !existed {
        git(main, &["rev-parse", "--verify", "-q", "HEAD"], GIT_BUDGET)
            .map_err(|_| "no commits yet: a worktree needs a commit to branch from".to_string())?;
        let has_branch = git(
            main,
            &[
                "rev-parse",
                "--verify",
                "-q",
                &format!("refs/heads/{branch}"),
            ],
            GIT_BUDGET,
        )
        .is_ok();
        let p = path.display().to_string();
        let args: Vec<&str> = if has_branch {
            vec!["worktree", "add", &p, &branch]
        } else {
            vec!["worktree", "add", "-b", &branch, &p]
        };
        git(main, &args, GIT_BUDGET)?;
    }
    let copied = copy_included(main, &path)?;
    Ok(Created {
        path,
        branch,
        existed,
        copied,
    })
}

/// Why a worktree may not be removed right now. Each is something the person can see and
/// clear; none is decided for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holding {
    /// `git status --porcelain` lines: uncommitted work.
    Dirty(Vec<String>),
    /// The harness's lock reason (a live `claude` session holds the worktree).
    Locked(String),
    /// A tmux session Air would attach the owner to.
    Tmux(String),
}

impl std::fmt::Display for Holding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Holding::Dirty(files) => write!(
                f,
                "uncommitted work: {}",
                files.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
            ),
            Holding::Locked(why) => write!(f, "locked: {why}"),
            Holding::Tmux(s) => write!(f, "live tmux session {s}"),
        }
    }
}

/// What is holding the worktree, in the order a person would clear it.
pub fn holdings(main: &Path, name: &str) -> Result<Vec<Holding>, String> {
    let path = dir_for(main, name);
    let mut held = Vec::new();
    let porcelain = git(&path, &["status", "--porcelain"], GIT_BUDGET)?;
    let dirty: Vec<String> = porcelain
        .lines()
        .map(|l| l.trim_start().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if !dirty.is_empty() {
        held.push(Holding::Dirty(dirty));
    }
    let list = git(main, &["worktree", "list", "--porcelain"], GIT_BUDGET)?;
    let mut in_ours = false;
    for line in list.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            in_ours = Path::new(p) == path
                || Path::new(p).canonicalize().ok() == path.canonicalize().ok();
        } else if in_ours && let Some(why) = line.strip_prefix("locked") {
            held.push(Holding::Locked(why.trim().to_string()));
        }
    }
    let session = super::tmux::session_name(&super::tmux::project_prefix(main), name);
    if super::tmux::sessions().contains(&session) {
        held.push(Holding::Tmux(session));
    }
    Ok(held)
}

/// Remove a worker's worktree. Refused, naming every holding, unless the tree is clean, no
/// harness lock is on it and no tmux session is attached to it. The branch stays.
pub fn remove(main: &Path, name: &str) -> Result<String, String> {
    let path = dir_for(main, name);
    if !is_worktree(main, &path) {
        return Err(format!(
            "{} is not a worktree of this repository",
            path.display()
        ));
    }
    let held = holdings(main, name)?;
    if !held.is_empty() {
        let why: Vec<String> = held.iter().map(ToString::to_string).collect();
        return Err(format!(
            "refused: {} is held ({}). Commit or discard the work, end the session, then \
             `air worker {name} --remove`; `git worktree remove --force {}` discards it.",
            path.display(),
            why.join("; "),
            path.display()
        ));
    }
    git(
        main,
        &["worktree", "remove", &path.display().to_string()],
        GIT_BUDGET,
    )?;
    Ok(format!(
        "removed {} (branch {} kept)",
        path.display(),
        branch_for(name)
    ))
}

/// `air worker <name> --remove`.
pub fn remove_cmd(repo: &Path, name: Option<&str>) -> i32 {
    let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) else {
        eprintln!("air worker --remove: name the worker");
        return 1;
    };
    match remove(&main_checkout(repo), name) {
        Ok(msg) => {
            println!("{msg}");
            0
        }
        Err(e) => {
            eprintln!("air worker --remove: {e}");
            2
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// gitignore's depth rule survives the translation: no slash means any depth, a slash
    /// anchors, a trailing slash is the directory's contents.
    #[test]
    fn pathspecs_follow_gitignore_depth_rules() {
        assert_eq!(pathspec_for(".env").as_deref(), Some(":(glob)**/.env"));
        assert_eq!(
            pathspec_for("backend/.env").as_deref(),
            Some(":(glob)backend/.env")
        );
        assert_eq!(
            pathspec_for("backend/keys/*.pem").as_deref(),
            Some(":(glob)backend/keys/*.pem")
        );
        assert_eq!(pathspec_for("/keys").as_deref(), Some(":(glob)keys"));
        assert_eq!(pathspec_for("keys/").as_deref(), Some(":(glob)**/keys/**"));
        assert_eq!(pathspec_for("a/keys/").as_deref(), Some(":(glob)a/keys/**"));
        assert_eq!(pathspec_for("  # comment"), None);
        assert_eq!(pathspec_for(""), None);
        // Negation is reported by `include_specs`, never silently matched as a literal.
        let (specs, unsupported) = include_specs("# x\n\n.env\n!keep.env\nbackend/keys/*.pem\n");
        assert_eq!(specs, [":(glob)**/.env", ":(glob)backend/keys/*.pem"]);
        assert_eq!(unsupported, ["!keep.env"]);
    }

    #[test]
    fn layout_matches_the_harness() {
        assert_eq!(
            dir_for(Path::new("/r"), "w1"),
            PathBuf::from("/r/.claude/worktrees/w1")
        );
        assert_eq!(branch_for("w1"), "worktree-w1");
    }
}
