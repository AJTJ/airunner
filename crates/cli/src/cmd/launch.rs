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
//! A `--task` never rides in argv. It is written to `<main>/.air/tasks/<name>.md` and the
//! prompt claude receives is a fixed sentence naming that path (air-er0: adopter's seven
//! worker deaths of 2026-08-30 were `pkill -f "air record verify"` matching the task prompt in
//! every peer's command line; `ps -o command=` showed the whole prompt). What still sits in
//! argv is Air's own fixed text: the roles path, the deny patterns, and until air-9dg the
//! `--settings` env blob. Decided and recorded rather than silently left: the deny patterns are
//! chosen by Air, not arbitrary, and the one plausible collision is `pkill -f "git push"`. They
//! move to a settings file the day `permissions.deny` in `--settings` is verified to hold in
//! every permission mode (the reason `--disallowed-tools` was chosen), or the first time a
//! worker dies to a pattern matching one of them, whichever comes first. The file indirection
//! itself goes when claude can take its first prompt from a file or stdin under tmux.
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
    "Bash(air close *)",
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

/// Deny rules for the coordinator: it steers and writes on main, but nothing it does reaches
/// a remote.
///
/// `Bash(git commit *)` was here until 2026-08-29 and is gone by the owner's ruling (air-iy1).
/// The incident: the coordinator wrote plan 0008, a decisions entry and a CLAUDE.md index row,
/// could not commit them, and the owner committed by hand — the owner doing a chore the
/// coordinator was in the middle of.
///
/// **The boundary the owner drew is the remote, not main.** `air land` already merges into
/// main and is already the coordinator's, so the deny was never protecting main from the
/// coordinator; it was stopping it from saving its own prose.
///
/// This is NOT precedent from adopter, and the bead's original framing that it was does not
/// survive checking. Their coordinator cannot hand-commit on main either, and their CLAUDE.md
/// forbids `git add`/`git commit` in the main checkout outright; what was allowed there was a
/// scripted path (`land.sh`: refuse, digest, `--no-ff` merge, verify, rewind on red), which is
/// what `air land` already is. They have never tried this, so they are evidence neither way.
/// It rests on the owner's ruling alone.
///
/// **Removal**: if a coordinator commit ever lands something on main that no worker branch
/// carried and no landing recorded, this comes back — and `air land` stays the route for
/// landing a worker's branch regardless. Hand-committing is for the coordinator's own prose.
pub const COORDINATOR_DENY: &[&str] = &["Bash(git push *)"];

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

/// Write the task to `<main>/.air/tasks/<name>.md` and return the path. Overwritten on every
/// launch of the same name: a worker outlives its bead, and the file is the CURRENT task.
fn task_file(repo: &Path, name: &str, task: &str) -> Result<std::path::PathBuf, String> {
    let dir = air_ledger::paths::air_dir_for(repo)
        .map_err(|e| e.to_string())?
        .join("tasks");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("{name}.md"));
    let mut body = task.trim_end().to_string();
    body.push('\n');
    std::fs::write(&path, body).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Pure: the prompt that stands in for the task in argv (air-er0). Fixed words plus a path,
/// so nothing a person typed into `--task` is ever in a process's command line.
pub fn task_prompt(path: &Path) -> String {
    format!(
        "Your task is in {}. Read that file and carry it out.",
        path.display()
    )
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
pub fn worker_argv(name: &str, project: &str, roles: &Path, extra: &[String]) -> Vec<String> {
    // AIR_ENFORCE=1: the hand-over gate denies instead of advising (air-i59; first bypass of
    // the advisory gate 2026-08-22 06:00). Coordinator launches do not set it.
    // AIR_PROJECT: which fleet this session may touch (air-0lk); both roles set it.
    let settings = serde_json::json!({
        "env": {
            "AIR_ROLE": "worker",
            "BEADS_ACTOR": name,
            "AIR_ENFORCE": "1",
            "AIR_PROJECT": project,
        }
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
    let mut base = worker_argv(name, &super::tmux::project_prefix(repo), roles, &[]);
    base.extend(repo_deny(repo, "worker_deny"));
    base.extend(extra.iter().cloned());
    base
}

/// Pure: the argv for the coordinator session.
pub fn coordinator_argv(
    project: &str,
    roles: &Path,
    channels_flag: &str,
    extra: &[String],
) -> Vec<String> {
    // No AIR_ENFORCE: the hand-over gate is the worker's. AIR_PROJECT is both roles' (air-0lk);
    // the coordinator is the one that can see every fleet on the machine.
    let settings = serde_json::json!({"env": {"AIR_ROLE": "coordinator", "AIR_PROJECT": project}});
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
    let mut base = coordinator_argv(&super::tmux::project_prefix(repo), roles, flag, &[]);
    base.extend(repo_deny(repo, "coordinator_deny"));
    base.extend(extra.iter().cloned());
    base
}

fn claude_bin() -> String {
    std::env::var("AIR_CLAUDE_BIN").unwrap_or_else(|_| "claude".into())
}

/// POSIX single-quote an argument so `sh -c` reconstructs it byte for byte. Bare words pass
/// through; anything else is wrapped in `'...'` with embedded `'` as `'\''`.
pub fn shell_quote(a: &str) -> String {
    let bare = !a.is_empty()
        && a.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./=:@%+,".contains(&b));
    if bare {
        a.to_string()
    } else {
        format!("'{}'", a.replace('\'', "'\\''"))
    }
}

/// Pure: the `--print` rendering of an exec, one shell line that yields the same argv.
pub fn print_line(bin: &str, argv: &[String]) -> String {
    std::iter::once(bin)
        .chain(argv.iter().map(String::as_str))
        .map(shell_quote)
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
///
/// The session is `<project>-<worker>`, not `<worker>`: `tmux ls` is machine-wide, so with two
/// fleets running the list said nothing about which project a pane belonged to (air-5lg).
fn spawn_detached(repo: &Path, name: &str, argv: &[String], print: bool) -> i32 {
    let bin = claude_bin();
    let socket = tmux_socket();
    let session = super::tmux::session_name(&super::tmux::project_prefix(repo), name);
    let targv = tmux_detached_argv(&session, repo, socket.as_deref(), &bin, argv);
    if print {
        println!("{}", print_line("tmux", &targv));
        return 0;
    }
    match Command::new("tmux").args(&targv).current_dir(repo).status() {
        Ok(s) if s.success() => {
            let l = socket
                .as_deref()
                .map(|s| format!("-L {s} "))
                .unwrap_or_default();
            println!("started tmux session {session} (stdin is not a tty; detached)");
            println!("attach: tmux {l}attach -t {session}");
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
        println!("{}", print_line(&bin, argv));
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

/// Worker argv with an initial prompt and, if `tmux`, `--tmux` (an attachable pane the
/// owner can open; the coordinator may launch workers this way, owner ruling 2026-08-21).
/// `--tmux` requires `--worktree` (cli-reference, accessed 2026-08-21), which workers always
/// have. `AIR_TMUX_MODE=classic` forces plain tmux outside iTerm2.
///
/// `prompt` is [`task_prompt`] (the sentence naming the task file), never the task itself.
///
/// The prompt goes *first*. `--disallowed-tools` takes space-separated values (cli-reference,
/// https://code.claude.com/docs/en/cli-reference, accessed 2026-08-22: example
/// `"Bash(git log *)" "Bash(git diff *)" "Edit"`), so a positional appended after the deny
/// list is read as one more deny rule, not as the prompt (air-2ct: adopter 2026-08-22,
/// three workers idle at an empty prompt once `--tmux`, the only thing terminating the
/// list, was stripped). The reference shows the prompt positional before flags
/// (`claude -p "query" --output-format json`).
pub fn worker_argv_tmux(
    base: Vec<String>,
    tmux: bool,
    mode: Option<&str>,
    prompt: Option<&str>,
) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(t) = prompt.filter(|t| !t.trim().is_empty()) {
        v.push(t.to_string());
    }
    v.extend(base);
    if tmux {
        v.push(tmux_flag(mode));
    }
    v
}

/// `AIR_TMUX_MODE`, read once at the edge so everything below it is decided from arguments.
fn tmux_mode() -> Option<String> {
    std::env::var("AIR_TMUX_MODE").ok()
}

/// Pure: the `--tmux` flag, with the mode passed in rather than read (air-7ah). Its test
/// asserted the flag was exactly `--tmux`, which is true only while the ambient
/// `AIR_TMUX_MODE` happens to be unset.
pub fn tmux_flag(mode: Option<&str>) -> String {
    match mode.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => format!("--tmux={m}"),
        None => "--tmux".to_string(),
    }
}

/// Pure: does claude read `task` in `argv` as the prompt? False when it sits in the value
/// run of a variadic list (`--disallowed-tools`, `--allowed-tools`) with no flag between.
pub fn task_is_prompt(argv: &[String], task: &str) -> bool {
    let Some(i) = argv.iter().position(|a| a == task) else {
        return false;
    };
    // Walk back to the nearest flag; if it is variadic, the task is one of its values.
    match argv.iter().take(i).rev().find(|a| a.starts_with("--")) {
        Some(f) => !matches!(
            f.as_str(),
            "--disallowed-tools" | "--disallowedTools" | "--allowed-tools" | "--allowedTools"
        ),
        None => true,
    }
}

/// Worker names a coordinator did not choose: `w1`, `w2`, … skipping every existing worktree
/// and live tmux session. A worker outlives its bead (tty-fix worked six), so the bead never
/// belongs in the name, and a coordinator that has a semantically useful name should still
/// pass one (owner ruling, 2026-08-22, air-5lg).
fn auto_worker_name(repo: &Path) -> String {
    let taken: Vec<String> = crate::git::worktrees(repo)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(p, _)| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    super::tmux::next_free_worker_name(&taken, &super::tmux::project_prefix(repo))
}

pub fn worker(
    repo: &Path,
    name: Option<&str>,
    extra: &[String],
    tmux: bool,
    task: Option<&str>,
    print: bool,
) -> i32 {
    let owned;
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n,
        None => {
            owned = auto_worker_name(repo);
            eprintln!("air worker: no name given; using {owned}");
            &owned
        }
    };
    if name == "main" || name.contains('/') {
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
    // The task goes to a file; argv gets a fixed sentence naming it (air-er0). Written under
    // `--print` too, so the printed command is one that runs.
    let prompt = match task.filter(|t| !t.trim().is_empty()) {
        Some(t) => match task_file(repo, name, t) {
            Ok(p) => Some(task_prompt(&p)),
            Err(e) => {
                eprintln!("air worker: {e}");
                return 1;
            }
        },
        None => None,
    };
    match launch_mode(std::io::stdin().is_terminal(), true) {
        Launch::Exec => {
            argv = worker_argv_tmux(argv, true, tmux_mode().as_deref(), prompt.as_deref());
            exec_claude(repo, &argv, print)
        }
        Launch::Detached => {
            // tmux is ours here, so claude gets no `--tmux`; the prompt still goes first
            // (air-2ct: after the deny list it reads as one more deny rule).
            argv = worker_argv_tmux(argv, false, None, prompt.as_deref());
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
            "air",
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
        assert_eq!(settings["env"]["AIR_ENFORCE"], "1");
        // air-0lk: which fleet this session may touch.
        assert_eq!(settings["env"]["AIR_PROJECT"], "air");
        let i = v.iter().position(|a| a == "--disallowed-tools").unwrap();
        assert_eq!(&v[i + 1..i + 1 + WORKER_DENY.len()], WORKER_DENY);
        assert_eq!(&v[v.len() - 2..], ["--model", "x"]);
    }

    /// The prompt must come before `--disallowed-tools`, whose values are space-separated
    /// (cli-reference, accessed 2026-08-22); after it, claude reads the task as a deny rule.
    #[test]
    fn task_precedes_the_deny_list_and_tmux_is_last() {
        let base = worker_argv("w", "air", Path::new("/r/roles.md"), &[]);
        let v = worker_argv_tmux(base.clone(), true, None, Some("fix fd-1 end to end"));
        assert_eq!(v[0], "fix fd-1 end to end");
        assert_eq!(v.last().map(String::as_str), Some("--tmux"));
        let deny = v.iter().position(|a| a == "--disallowed-tools").unwrap();
        assert!(deny > 0, "task must not follow the variadic deny list");
        // Without --tmux (a detached launch) the task is still the prompt, not a deny value.
        let v = worker_argv_tmux(base.clone(), false, None, Some("say hello"));
        assert_eq!(v[0], "say hello");
        assert!(!v.contains(&"--tmux".to_string()));
        assert_eq!(&v[1..], &base[..]);
        let v = worker_argv_tmux(base, true, None, None);
        assert_eq!(v.last().map(String::as_str), Some("--tmux"));
        assert_eq!(v[0], "--worktree");
        // air-7ah: the mode is decided from what it is given, not from ambient AIR_TMUX_MODE,
        // which is what made the assertions above true only by accident of the environment.
        assert_eq!(tmux_flag(None), "--tmux");
        assert_eq!(tmux_flag(Some("")), "--tmux");
        assert_eq!(tmux_flag(Some("classic")), "--tmux=classic");
    }

    /// air-er0: the prompt names the file and carries none of the task.
    #[test]
    fn task_prompt_carries_the_path_and_none_of_the_task() {
        let p = task_prompt(Path::new("/r/.air/tasks/w1.md"));
        assert_eq!(
            p,
            "Your task is in /r/.air/tasks/w1.md. Read that file and carry it out."
        );
        assert!(task_is_prompt(
            &worker_argv_tmux(vec![], false, None, Some(&p)),
            &p
        ));
    }

    /// `--print` pasted into `sh -c` must reproduce the exec argv for a task with a space,
    /// a single quote, and a `$`.
    #[test]
    fn print_line_round_trips_through_sh() {
        let task = "fix it's $HOME \"now\"";
        let argv = worker_argv_tmux(
            worker_argv("w", "air", Path::new("/r/roles.md"), &[]),
            true,
            None,
            Some(task),
        );
        let line = print_line("claude", &argv);
        let script = format!("{} \"$@\"", "printf '%s\\0'");
        let out = Command::new("sh")
            .arg("-c")
            .arg(format!("set -- {}; {script}", line))
            .output()
            .unwrap();
        assert!(out.status.success());
        let got: Vec<&str> = std::str::from_utf8(&out.stdout)
            .unwrap()
            .split('\0')
            .filter(|s| !s.is_empty())
            .collect();
        let want: Vec<&str> = std::iter::once("claude")
            .chain(argv.iter().map(String::as_str))
            .collect();
        assert_eq!(got, want);
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
        let v = coordinator_argv("air", Path::new("/r/.air/roles.md"), "--channels", &[]);
        assert_eq!(&v[..2], ["--channels", "server:air"]);
        // The remote is the boundary, not main (air-iy1): push denied, commit allowed.
        assert!(v.contains(&"Bash(git push *)".to_string()));
        assert!(!v.contains(&"Bash(git commit *)".to_string()));
        assert!(!v.contains(&"--worktree".to_string()));
        let settings: serde_json::Value =
            serde_json::from_str(&v[v.iter().position(|a| a == "--settings").unwrap() + 1])
                .unwrap();
        assert!(settings["env"].get("AIR_ENFORCE").is_none());
        // But the project fence is both roles' (air-0lk).
        assert_eq!(settings["env"]["AIR_PROJECT"], "air");
    }
}
