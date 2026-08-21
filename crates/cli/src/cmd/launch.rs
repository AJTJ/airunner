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
//! https://code.claude.com/docs/en/cli-reference (accessed 2026-08-20); the channel flag is
//! `--channels server:<name>` (override with `AIR_CHANNELS_FLAG` while the feature is in
//! preview, e.g. `--dangerously-load-development-channels`).

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
    "EnterWorktree",
    "ExitWorktree",
];

/// Deny rules for the coordinator: it steers, it does not commit on main or push.
pub const COORDINATOR_DENY: &[&str] = &["Bash(git push *)", "Bash(git commit *)"];

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

fn exec_claude(repo: &Path, argv: &[String], print: bool) -> i32 {
    let bin = std::env::var("AIR_CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
    if print {
        let shown: Vec<String> = std::iter::once(bin.clone())
            .chain(argv.iter().cloned())
            .map(|a| {
                if a.contains(' ') || a.contains('"') {
                    format!("'{a}'")
                } else {
                    a
                }
            })
            .collect();
        println!("{}", shown.join(" "));
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

pub fn worker(repo: &Path, name: &str, extra: &[String], print: bool) -> i32 {
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
    exec_claude(repo, &worker_argv(name, &roles, extra), print)
}

pub fn coordinator(repo: &Path, extra: &[String], print: bool) -> i32 {
    let roles = match roles_file(repo) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("air coordinator: {e}");
            return 1;
        }
    };
    let flag = std::env::var("AIR_CHANNELS_FLAG").unwrap_or_else(|_| "--channels".into());
    exec_claude(repo, &coordinator_argv(&roles, &flag, extra), print)
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
    fn coordinator_argv_attaches_the_channel() {
        let v = coordinator_argv(Path::new("/r/.air/roles.md"), "--channels", &[]);
        assert_eq!(&v[..2], ["--channels", "server:air"]);
        assert!(v.contains(&"Bash(git commit *)".to_string()));
        assert!(!v.contains(&"--worktree".to_string()));
    }
}
