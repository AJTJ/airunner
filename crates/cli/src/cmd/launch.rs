//! `air worker <name>` and `air coordinator`: start an *interactive* Claude Code session in
//! this terminal with the role applied (decisions 2026-08-20: a human is always in the loop;
//! launchers never start headless sessions).
//!
//! Worker: `claude --worktree <name>` (native isolation from main), the roles prose appended
//! to the system prompt, a deny list that holds in every permission mode, and env that used
//! to drift in per-worktree files (`AIR_ROLE`, `BEADS_ACTOR`).
//! Coordinator: `claude` in the main checkout with the Air channel attached so attention
//! conditions are delivered into the session.
//!
//! `--print` shows the exact command instead of running it. Flags verified against
//! https://code.claude.com/docs/en/cli-reference (accessed 2026-08-20). The channel flag is
//! `--dangerously-load-development-channels server:air` (a local server is not on the
//! allowlist; `--channels` rejects it, seen live 2026-08-21); `AIR_CHANNELS_FLAG` overrides.

use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

use crate::cmd::install::ROLES_MD;

/// Deny rules for a worker (roles.md "Never"; agent-roles-and-confinement §5 L1).
pub const WORKER_DENY: &[&str] = &[
    "Bash(air land *)",
    "Bash(git push *)",
    "Bash(bd create *)",
    "Bash(bd sync *)",
    "Bash(bd update *--claim*)",
    "Bash(claude *)",
    "Bash(air worker *)",
    "Bash(air coordinator *)",
    "EnterWorktree",
    "ExitWorktree",
];

/// Deny rules for the coordinator: it steers, it does not commit on main or push.
pub const COORDINATOR_DENY: &[&str] = &["Bash(git push *)", "Bash(git commit *)"];

/// Repo-specific deny rules, tracked in `<main>/.claude/air.json`:
/// `{"worker_deny": ["Bash(make deploy*)"], "coordinator_deny": [...]}`. Patterns, not
/// enumerations, so a new publish target cannot ship outside the list (adopter capture
/// fcd8ff: `make deploy-site` shipped without being added to a list that named `deploy-api`).
pub fn repo_deny(repo: &Path, key: &str) -> Vec<String> {
    let Ok(air_dir) = air_ledger::paths::air_dir_for(repo) else {
        return Vec::new();
    };
    let path = air_dir
        .parent()
        .map(|m| m.join(".claude/air.json"))
        .unwrap_or_default();
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get(key).and_then(|a| a.as_array()).cloned())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn roles_file(repo: &Path) -> Result<std::path::PathBuf, String> {
    let dir = air_ledger::paths::air_dir_for(repo).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("roles.md");
    // Always refresh: the embedded copy is the source of truth for this binary's version.
    std::fs::write(&path, ROLES_MD).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Pure: the argv for a worker session.
pub fn worker_argv(name: &str, roles: &Path, extra: &[String]) -> Vec<String> {
    let settings = serde_json::json!({
        "env": {"AIR_ROLE": "worker", "BEADS_ACTOR": name}
    });
    let mut v: Vec<String> = vec![
        "--worktree".into(),
        name.into(),
        "--append-system-prompt-file".into(),
        roles.display().to_string(),
        "--settings".into(),
        settings.to_string(),
        "--disallowed-tools".into(),
    ];
    v.extend(WORKER_DENY.iter().map(|s| (*s).to_string()));
    v.extend(extra.iter().cloned());
    v
}

/// Worker argv including the repo's own deny rules (inserted before any pass-through args).
fn worker_argv_for(repo: &Path, name: &str, roles: &Path, extra: &[String]) -> Vec<String> {
    let mut base = worker_argv(name, roles, &[]);
    base.extend(repo_deny(repo, "worker_deny"));
    base.extend(extra.iter().cloned());
    base
}

/// Pure: the argv for the coordinator session.
pub fn coordinator_argv(roles: &Path, channels_flag: &str, extra: &[String]) -> Vec<String> {
    let settings = serde_json::json!({"env": {"AIR_ROLE": "coordinator"}});
    let mut v: Vec<String> = vec![
        channels_flag.into(),
        "server:air".into(),
        "--append-system-prompt-file".into(),
        roles.display().to_string(),
        "--settings".into(),
        settings.to_string(),
        "--disallowed-tools".into(),
    ];
    v.extend(COORDINATOR_DENY.iter().map(|s| (*s).to_string()));
    v.extend(extra.iter().cloned());
    v
}

fn coordinator_argv_for(repo: &Path, roles: &Path, flag: &str, extra: &[String]) -> Vec<String> {
    let mut base = coordinator_argv(roles, flag, &[]);
    base.extend(repo_deny(repo, "coordinator_deny"));
    base.extend(extra.iter().cloned());
    base
}

fn claude_bin() -> String {
    std::env::var("AIR_CLAUDE_BIN").unwrap_or_else(|_| "claude".into())
}

fn shell_quote(bin: &str, argv: &[String]) -> String {
    std::iter::once(bin.to_string())
        .chain(argv.iter().cloned())
        .map(|a| {
            if a.contains(' ') || a.contains('"') {
                format!("'{a}'")
            } else {
                a
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// How a worker session is started. Pure decision so the selftest probe can exercise it.
#[derive(Debug, PartialEq, Eq)]
pub enum Launch {
    /// Replace this process: the caller's terminal talks to claude directly.
    Exec,
    /// No controlling tty (stdin is a socket: the coordinator's Bash tool, `</dev/null`), so
    /// `claude --tmux` cannot run here (`tcgetattr failed: Operation not supported on
    /// socket`, adopter 2026-08-22, backlog #19). Start a detached tmux session instead.
    Detached,
}

pub fn launch_mode(stdin_is_tty: bool, tmux_requested: bool) -> Launch {
    if tmux_requested && !stdin_is_tty {
        Launch::Detached
    } else {
        Launch::Exec
    }
}

/// Pure: `tmux new-session -d -s <name> -c <repo> -- <bin> <argv...>`. `socket` (from
/// `AIR_TMUX_SOCKET`) becomes `-L <socket>` so tests never touch the user's tmux server.
pub fn tmux_detached_argv(
    name: &str,
    repo: &Path,
    socket: Option<&str>,
    bin: &str,
    argv: &[String],
) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if let Some(s) = socket.filter(|s| !s.is_empty()) {
        v.push("-L".into());
        v.push(s.into());
    }
    v.extend(
        [
            "new-session",
            "-d",
            "-s",
            name,
            "-c",
            &repo.display().to_string(),
            "--",
            bin,
        ]
        .map(String::from),
    );
    v.extend(argv.iter().cloned());
    v
}

fn tmux_socket() -> Option<String> {
    std::env::var("AIR_TMUX_SOCKET")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Start `claude` in a detached tmux session and return without touching the caller's
/// terminal. Prints the session name and the attach command.
fn spawn_detached(repo: &Path, name: &str, argv: &[String], print: bool) -> i32 {
    let bin = claude_bin();
    let socket = tmux_socket();
    let targv = tmux_detached_argv(name, repo, socket.as_deref(), &bin, argv);
    if print {
        println!("{}", shell_quote("tmux", &targv));
        return 0;
    }
    match Command::new("tmux").args(&targv).current_dir(repo).status() {
        Ok(s) if s.success() => {
            let l = socket
                .as_deref()
                .map(|s| format!("-L {s} "))
                .unwrap_or_default();
            println!("started tmux session {name} (stdin is not a tty; detached)");
            println!("attach: tmux {l}attach -t {name}");
            0
        }
        Ok(s) => {
            eprintln!("air worker: tmux exited with {s}");
            1
        }
        Err(e) => {
            eprintln!("air worker: could not run tmux: {e}");
            1
        }
    }
}

fn exec_claude(repo: &Path, argv: &[String], print: bool) -> i32 {
    let bin = claude_bin();
    if print {
        println!("{}", shell_quote(&bin, argv));
        return 0;
    }
    let mut cmd = Command::new(&bin);
    cmd.args(argv).current_dir(repo);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Replace this process: the terminal talks to claude directly (human in the loop).
        let err = cmd.exec();
        eprintln!("air: could not exec {bin}: {err}");
        1
    }
    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(s) => s.code().unwrap_or(1),
            Err(e) => {
                eprintln!("air: could not run {bin}: {e}");
                1
            }
        }
    }
}

/// Worker argv with `--tmux` (an attachable pane the owner can open; the coordinator may
/// launch workers this way, owner ruling 2026-08-21) and an initial task as the prompt.
/// `--tmux` requires `--worktree` (cli-reference, accessed 2026-08-21), which workers always
/// have. `AIR_TMUX_MODE=classic` forces plain tmux outside iTerm2.
pub fn worker_argv_tmux(base: Vec<String>, task: Option<&str>) -> Vec<String> {
    let mut v = base;
    let mode = std::env::var("AIR_TMUX_MODE").ok();
    v.push(match mode.as_deref() {
        Some(m) if !m.is_empty() => format!("--tmux={m}"),
        _ => "--tmux".to_string(),
    });
    if let Some(t) = task.filter(|t| !t.trim().is_empty()) {
        v.push(t.to_string());
    }
    v
}

pub fn worker(
    repo: &Path,
    name: &str,
    extra: &[String],
    tmux: bool,
    task: Option<&str>,
    print: bool,
) -> i32 {
    if name.is_empty() || name == "main" || name.contains('/') {
        eprintln!("air worker: name must be a worktree name (not `main`, no slashes)");
        return 1;
    }
    let roles = match roles_file(repo) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("air worker: {e}");
            return 1;
        }
    };
    let mut argv = worker_argv_for(repo, name, &roles, extra);
    if !(tmux || task.is_some()) {
        return exec_claude(repo, &argv, print);
    }
    match launch_mode(std::io::stdin().is_terminal(), true) {
        Launch::Exec => {
            argv = worker_argv_tmux(argv, task);
            exec_claude(repo, &argv, print)
        }
        Launch::Detached => {
            // tmux is ours here, so claude gets no `--tmux`; the task stays the first prompt.
            if let Some(t) = task.filter(|t| !t.trim().is_empty()) {
                argv.push(t.to_string());
            }
            spawn_detached(repo, name, &argv, print)
        }
    }
}

pub fn coordinator(repo: &Path, extra: &[String], print: bool) -> i32 {
    let roles = match roles_file(repo) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("air coordinator: {e}");
            return 1;
        }
    };
    // A local `.mcp.json` server is not on Claude Code's channel allowlist; the preview flag
    // is required (verified live on 2.1.239, 2026-08-21: "server air is not on the approved
    // channels allowlist (use --dangerously-load-development-channels for local dev)").
    let flag = std::env::var("AIR_CHANNELS_FLAG")
        .unwrap_or_else(|_| "--dangerously-load-development-channels".into());
    exec_claude(
        repo,
        &coordinator_argv_for(repo, &roles, &flag, extra),
        print,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn worker_argv_carries_isolation_prose_env_and_denies() {
        let v = worker_argv(
            "frontend",
            Path::new("/r/.air/roles.md"),
            &["--model".into(), "x".into()],
        );
        assert_eq!(&v[..2], ["--worktree", "frontend"]);
        assert!(
            v.windows(2)
                .any(|w| w[0] == "--append-system-prompt-file" && w[1] == "/r/.air/roles.md")
        );
        let settings: serde_json::Value =
            serde_json::from_str(&v[v.iter().position(|a| a == "--settings").unwrap() + 1])
                .unwrap();
        assert_eq!(settings["env"]["BEADS_ACTOR"], "frontend");
        let i = v.iter().position(|a| a == "--disallowed-tools").unwrap();
        assert_eq!(&v[i + 1..i + 1 + WORKER_DENY.len()], WORKER_DENY);
        assert_eq!(&v[v.len() - 2..], ["--model", "x"]);
    }

    #[test]
    fn tmux_adds_the_flag_and_the_task_last() {
        let base = worker_argv("w", Path::new("/r/roles.md"), &[]);
        let v = worker_argv_tmux(base.clone(), Some("fix fd-1 end to end"));
        assert_eq!(&v[v.len() - 2..], ["--tmux", "fix fd-1 end to end"]);
        let v = worker_argv_tmux(base, None);
        assert_eq!(v.last().map(String::as_str), Some("--tmux"));
    }

    #[test]
    fn socket_stdin_with_tmux_is_detached_tty_execs() {
        assert_eq!(launch_mode(false, true), Launch::Detached);
        assert_eq!(launch_mode(true, true), Launch::Exec);
        assert_eq!(launch_mode(false, false), Launch::Exec);
    }

    #[test]
    fn detached_argv_never_passes_tmux_to_claude() {
        let argv = vec!["--worktree".to_string(), "w".into(), "do x".into()];
        let v = tmux_detached_argv("w", Path::new("/r"), Some("air-test"), "claude", &argv);
        assert_eq!(
            v,
            [
                "-L",
                "air-test",
                "new-session",
                "-d",
                "-s",
                "w",
                "-c",
                "/r",
                "--",
                "claude",
                "--worktree",
                "w",
                "do x"
            ]
        );
        assert!(!v.iter().any(|a| a.starts_with("--tmux")));
        let v = tmux_detached_argv("w", Path::new("/r"), None, "claude", &argv);
        assert_eq!(v[0], "new-session");
    }

    #[test]
    fn coordinator_argv_attaches_the_channel() {
        let v = coordinator_argv(Path::new("/r/.air/roles.md"), "--channels", &[]);
        assert_eq!(&v[..2], ["--channels", "server:air"]);
        assert!(v.contains(&"Bash(git commit *)".to_string()));
        assert!(!v.contains(&"--worktree".to_string()));
    }
}
