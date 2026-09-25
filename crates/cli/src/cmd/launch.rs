//! `air worker <name>` and `air coordinator`: start an *interactive* Claude Code session in
//! this terminal with the role applied (decisions 2026-08-20: a human is always in the loop;
//! launchers never start headless sessions).
//!
//! Worker: `claude` started IN the worktree Air made (air-fdz, air-8gj; no `--worktree`, and
//! the isolation is the cwd plus the PreToolUse fence in `air_hooks::fence`), the roles prose
//! appended to the system prompt, a deny list that holds in every permission mode, and env
//! that used to drift in per-worktree files (`AIR_ROLE`, `BEADS_ACTOR`).
//! Lane (`air lane`, air-jc2p.2): a worker launch with `AIR_ROLE=lane` and `air land` allowed.
//! Coordinator: `claude` in its own worktree, `.claude/worktrees/coordinator`, always in the
//! tmux session `<project>-coordinator` (air-jc2p.1), with the Air channel attached so
//! attention conditions are delivered into the session, plus Metis when the repo declares it
//! (air-g5o). The channel server is `.mcp.json`'s, which is tracked and so present in the
//! worktree, and the ledger it polls resolves through the git common dir to the main checkout's
//! `.air/` from any worktree (`air_ledger::paths::air_dir_for`).
//!
//! A `--task` never rides in argv. It is written to `<main>/.air/tasks/<name>.md` and the
//! prompt claude receives is a fixed sentence naming that path (air-er0: the adopter's seven
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
    // A worker that could start a lane could start a session that lands (air-jc2p.2).
    "Bash(air lane *)",
    "EnterWorktree",
    "ExitWorktree",
    // Owner, 2026-08-30 (air-bm3): a worker reaches the owner through `air capture`, which
    // the coordinator files as a bead labelled `owner`; that queue is visible in `air status`
    // and leaves a row. `AskUserQuestion` is a second path to the same authority, obtained at
    // will, outside the queue, with no trace of the question, the answer, or that a decision
    // was made — the provenance gap of 2026-08-29/30 (capture 01M181J9ZNH46M66ZCEJ26ABEV:
    // an instruction typed into a pane had to be confirmed by grepping a transcript). The
    // PreToolUse matcher carries `AskUserQuestion` from the same change, so an attempt is an
    // event line and the deny is countable. Remove when a round's PreToolUse events show zero
    // `AskUserQuestion` attempts by worker sessions WITH the matcher carrying it: a zero
    // before the matcher did was case 3b (the input never arrived) and settled nothing.
    "AskUserQuestion",
];

/// Deny rules for the verification lane: a worker's, minus `air land` (air-jc2p.2, owner
/// 2026-09-14: the lane lands, and landing is refused to every other role). Derived rather than
/// listed, so a rule added for workers reaches the lane without anyone remembering to.
/// Removed with the lane (roles.md, its removal condition).
pub fn lane_deny() -> Vec<&'static str> {
    WORKER_DENY
        .iter()
        .copied()
        .filter(|d| *d != "Bash(air land *)")
        .collect()
}

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
/// This is NOT precedent from the adopter, and the bead's original framing that it was does not
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
/// enumerations, so a new publish target cannot ship outside the list (the adopter's capture
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

/// The coordinator's own appended file: the role prose, plus the Metis split when Metis is
/// attached (air-g5o).
///
/// A SECOND file rather than a line in `.air/roles.md`, for two reasons. `roles.md` is
/// rewritten by whichever role launches last, so a paragraph put there for the coordinator
/// would be stripped by the next worker launch. And it says only what Air records and refuses
/// — a paragraph about one planning tool does not belong in a file every adopting repo gets,
/// including the ones with no Metis.
///
/// The split itself exists because Metis's own instruction text declares Metis the only system
/// of record and forbids plans outside it, which contradicts this repo (analysis 2026-09-05,
/// air-ate). Injecting a plugin's prose unchanged is how a tool's opinion becomes a rule
/// nobody chose.
fn coordinator_roles_file(repo: &Path, metis: bool) -> Result<std::path::PathBuf, String> {
    if !metis {
        return roles_file(repo);
    }
    let dir = air_ledger::paths::air_dir_for(repo).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("coordinator.md");
    let text = format!("{ROLES_MD}\n{}", super::install::METIS_SPLIT);
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

fn roles_file(repo: &Path) -> Result<std::path::PathBuf, String> {
    let dir = air_ledger::paths::air_dir_for(repo).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("roles.md");
    // Always refresh: the embedded copy is the source of truth for this binary's version.
    std::fs::write(&path, ROLES_MD).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// The env a worker session runs with, set on the SPAWNED PROCESS (`Command::env`, or
/// `tmux new-session -e` on the detached path) and not only in a `--settings` blob.
///
/// air-9dg: the adopter's workers carried two `--settings`; the second (added to turn off Remote
/// Control) replaced the first, so `AIR_ENFORCE` never reached a hook and the one refusal Air
/// promises advised instead of refusing for five hours, with nothing saying so. Nothing on a
/// command line can clobber a process environment. The blob stays too, merged with any
/// pass-through `--settings` ([`merge_settings`]), because whether `claude --tmux` carries the
/// launching process's env into the pane it opens is not verified; that path is the one
/// reason it is still there, and it goes when Air owns the tmux session on every path
/// (air-fdz) or the forwarding is verified.
///
/// AIR_ENFORCE=1: the hand-over gate denies instead of advising (air-i59; first bypass of the
/// advisory gate 2026-08-22 06:00). Coordinator launches do not set it.
/// AIR_PROJECT: which fleet this session may touch (air-0lk); both roles set it.
/// `role` is `worker` or `lane` (air-jc2p.2): the lane is enforced and named as a worker is.
pub fn role_env(role: &str, name: &str, project: &str) -> Vec<(String, String)> {
    [
        ("AIR_ROLE", role),
        ("BEADS_ACTOR", name),
        ("AIR_ENFORCE", "1"),
        ("AIR_PROJECT", project),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// The coordinator's env: no AIR_ENFORCE (the gate is the worker's), the project fence for
/// both roles (air-0lk).
pub fn coordinator_env(project: &str) -> Vec<(String, String)> {
    [("AIR_ROLE", "coordinator"), ("AIR_PROJECT", project)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn settings_blob(env: &[(String, String)]) -> String {
    let env: serde_json::Map<String, serde_json::Value> = env
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    serde_json::json!({ "env": env }).to_string()
}

/// Pure: pull every `--settings <v>` / `--settings=<v>` out of pass-through args. Inline JSON
/// objects come back to be merged; a file path is refused, naming what it would have dropped
/// (air-9dg: the defect was SILENT replacement; a loud refusal is acceptable, a merge better).
pub fn split_settings(extra: &[String]) -> Result<(Vec<String>, Vec<serde_json::Value>), String> {
    let mut rest = Vec::new();
    let mut blobs = Vec::new();
    let mut it = extra.iter();
    while let Some(a) = it.next() {
        let value = if a == "--settings" {
            it.next().cloned()
        } else if let Some(v) = a.strip_prefix("--settings=") {
            Some(v.to_string())
        } else {
            rest.push(a.clone());
            continue;
        };
        let Some(value) = value else {
            return Err("--settings given without a value".into());
        };
        match serde_json::from_str::<serde_json::Value>(&value) {
            Ok(v) if v.is_object() => blobs.push(v),
            _ => {
                return Err(format!(
                    "a pass-through `--settings {value}` would replace Air's own --settings, \
                     dropping AIR_ENFORCE, AIR_ROLE, AIR_PROJECT and BEADS_ACTOR from the \
                     session (air-9dg). Pass the settings as inline JSON and they are merged."
                ));
            }
        }
    }
    Ok((rest, blobs))
}

/// Pure: merge pass-through settings objects into the `--settings` blob already in `argv`,
/// so the command line carries ONE. Theirs first, ours on top: every key of theirs survives,
/// `env` is merged as a map, and Air's four env values win.
pub fn merge_settings(argv: &mut [String], theirs: &[serde_json::Value]) {
    let Some(i) = argv.iter().position(|a| a == "--settings") else {
        return;
    };
    let Some(slot) = argv.get_mut(i.saturating_add(1)) else {
        return;
    };
    let ours: serde_json::Value = serde_json::from_str(slot).unwrap_or_default();
    let mut merged = serde_json::Map::new();
    for t in theirs {
        if let Some(o) = t.as_object() {
            for (k, v) in o {
                if k == "env" {
                    let env = merged.entry("env").or_insert_with(|| serde_json::json!({}));
                    if let (Some(dst), Some(src)) = (env.as_object_mut(), v.as_object()) {
                        dst.extend(src.iter().map(|(k, v)| (k.clone(), v.clone())));
                    }
                } else {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }
    }
    if let Some(o) = ours.as_object() {
        for (k, v) in o {
            if k == "env" {
                let env = merged.entry("env").or_insert_with(|| serde_json::json!({}));
                if let (Some(dst), Some(src)) = (env.as_object_mut(), v.as_object()) {
                    dst.extend(src.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
            } else {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    *slot = serde_json::Value::Object(merged).to_string();
}

/// Pure: the Claude Code session name (`claude --name`, "Set a display name for this
/// session", `claude --help` on 2.1.272), which is what `ListAgents` shows and `SendMessage`
/// addresses. The same string as the tmux session, so one name finds a session everywhere:
/// `<project>-worker-1`, `<project>-lane`, `<project>-coordinator`. Before this Air set none and
/// the harness made one up (an adopter's showed a random word and a number), naming neither the role nor
/// the fleet (owner, 2026-09-25, air-jc2p.4).
pub fn display_name(project: &str, name: &str) -> String {
    super::tmux::session_name(project, name)
}

/// Names `air worker` will not take: they read as another role everywhere a name is shown
/// (`hook::role_for`), and `main` is the main checkout. Refused rather than accepted, because a
/// worker called `lane` would be listed as the lane while holding a worker's permissions.
const RESERVED: &[&str] = &["main", "coordinator", "lane"];

/// Pure: the argv for a worker session. No `--worktree` (air-8gj): Air creates the worktree
/// (air-fdz) and starts claude with its cwd inside it. The harness's own worktree isolation,
/// which the flag switched on, is off: in the adopter's record it stopped no observed write to
/// the main checkout and cost 455 refusals in five days, 388 of them (88%) with no git token in
/// the command, plus a native build refused with no prompt and permission prompts nobody could
/// answer unattended (`private/notes/2026-09-06-answers-worktree-and-verify.md`, owner
/// ruling 2026-09-06). What holds the line instead: the cwd, the deny list below, the
/// peer-on-file and hand-over hooks, and one PreToolUse denial of an Edit/Write whose resolved
/// path leaves the worktree (`hook.rs`, `air_hooks::fence`).
pub fn worker_argv(name: &str, project: &str, roles: &Path, extra: &[String]) -> Vec<String> {
    role_argv("worker", name, project, roles, WORKER_DENY, extra)
}

/// Pure: the lane's argv, a worker's with `AIR_ROLE=lane` and [`lane_deny`] (air-jc2p.2).
pub fn lane_argv(name: &str, project: &str, roles: &Path, extra: &[String]) -> Vec<String> {
    role_argv("lane", name, project, roles, &lane_deny(), extra)
}

fn role_argv(
    role: &str,
    name: &str,
    project: &str,
    roles: &Path,
    deny: &[&str],
    extra: &[String],
) -> Vec<String> {
    let settings = settings_blob(&role_env(role, name, project));
    let mut v: Vec<String> = vec![
        "--append-system-prompt-file".into(),
        roles.display().to_string(),
        "--settings".into(),
        settings.to_string(),
        "--name".into(),
        display_name(project, name),
        "--disallowed-tools".into(),
    ];
    v.extend(deny.iter().map(|s| (*s).to_string()));
    v.extend(extra.iter().cloned());
    v
}

/// Worker (or lane) argv including the repo's own deny rules (inserted before any pass-through
/// args), with any pass-through `--settings` merged into Air's so the line carries one
/// (air-9dg). The lane takes the repo's `worker_deny` too: it is a worker with one permission
/// added.
fn worker_argv_for(
    repo: &Path,
    role: &str,
    name: &str,
    roles: &Path,
    extra: &[String],
) -> Result<Vec<String>, String> {
    let (extra, theirs) = split_settings(extra)?;
    let project = super::tmux::project_prefix(repo);
    let mut base = if role == "lane" {
        lane_argv(name, &project, roles, &[])
    } else {
        worker_argv(name, &project, roles, &[])
    };
    merge_settings(&mut base, &theirs);
    base.extend(repo_deny(repo, "worker_deny"));
    base.extend(extra);
    Ok(base)
}

/// Pure: the argv for the coordinator session.
///
/// `metis` is whatever [`super::metis::argv`] returned: the MCP server, and the plugin
/// directory when the repo declared one that exists (air-g5o). It is placed BEFORE `extra`, so
/// a caller passing its own `--mcp-config` is last and wins, the same ordering every other
/// pass-through here already has. Workers get none of it: the coordinator plans, workers work.
pub fn coordinator_argv(
    project: &str,
    roles: &Path,
    channels_flag: &str,
    metis: &[String],
    extra: &[String],
) -> Vec<String> {
    // No AIR_ENFORCE: the hand-over gate is the worker's. AIR_PROJECT is both roles' (air-0lk);
    // the coordinator is the one that can see every fleet on the machine.
    let settings = settings_blob(&coordinator_env(project));
    let mut v: Vec<String> = vec![
        channels_flag.into(),
        "server:air".into(),
        "--append-system-prompt-file".into(),
        roles.display().to_string(),
        "--settings".into(),
        settings,
        "--name".into(),
        display_name(project, "coordinator"),
        "--disallowed-tools".into(),
    ];
    v.extend(COORDINATOR_DENY.iter().map(|s| (*s).to_string()));
    v.extend(metis.iter().cloned());
    v.extend(extra.iter().cloned());
    v
}

fn coordinator_argv_for(
    repo: &Path,
    roles: &Path,
    flag: &str,
    metis: &[String],
    extra: &[String],
) -> Result<Vec<String>, String> {
    let (extra, theirs) = split_settings(extra)?;
    let mut base = coordinator_argv(&super::tmux::project_prefix(repo), roles, flag, metis, &[]);
    merge_settings(&mut base, &theirs);
    base.extend(repo_deny(repo, "coordinator_deny"));
    base.extend(extra);
    Ok(base)
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

/// Pure: [`print_line`] with the process env in front as shell assignments, so the printed
/// line runs with the same environment the launcher would have set (air-9dg).
pub fn print_env_line(env: &[(String, String)], bin: &str, argv: &[String]) -> String {
    let mut parts: Vec<String> = env
        .iter()
        .map(|(k, v)| format!("{k}={}", shell_quote(v)))
        .collect();
    parts.push(print_line(bin, argv));
    parts.join(" ")
}

/// How a worker session is started. Pure decision so the selftest probe can exercise it.
#[derive(Debug, PartialEq, Eq)]
pub enum Launch {
    /// Replace this process: the caller's terminal talks to claude directly.
    Exec,
    /// No controlling tty (stdin is a socket: the coordinator's Bash tool, `</dev/null`), so
    /// `claude --tmux` cannot run here (`tcgetattr failed: Operation not supported on
    /// socket`, the adopter 2026-08-22, backlog #19). Start a detached tmux session instead.
    Detached,
}

pub fn launch_mode(stdin_is_tty: bool, tmux_requested: bool) -> Launch {
    if tmux_requested && !stdin_is_tty {
        Launch::Detached
    } else {
        Launch::Exec
    }
}

/// Pure: `tmux new-session [-d] -s <name> -e K=V… -c <cwd> -- <bin> <argv...>`. `socket`
/// (from `AIR_TMUX_SOCKET`) becomes `-L <socket>` so tests never touch the user's tmux server.
/// `detached` is the coordinator's path (no tty); without it the session opens in the
/// caller's terminal, which is what `claude --tmux` used to do and can no longer, since
/// `--tmux` requires `--worktree` and the flag is gone (air-8gj).
///
/// `-e` (tmux ≥ 3.2, 2021) is how env reaches a pane when the server already exists: a
/// running server hands new sessions ITS environment plus `update-environment`, not the
/// client's, so `Command::env` on the `tmux` client alone would set nothing (air-9dg).
pub fn tmux_session_argv(
    name: &str,
    cwd: &Path,
    socket: Option<&str>,
    env: &[(String, String)],
    detached: bool,
    bin: &str,
    argv: &[String],
) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if let Some(s) = socket.filter(|s| !s.is_empty()) {
        v.push("-L".into());
        v.push(s.into());
    }
    v.push("new-session".into());
    if detached {
        v.push("-d".into());
    }
    v.extend(["-s", name].map(String::from));
    let repo = cwd;
    for (k, val) in env {
        v.push("-e".into());
        v.push(format!("{k}={val}"));
    }
    v.extend(["-c", &repo.display().to_string(), "--", bin].map(String::from));
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
fn spawn_tmux(
    repo: &Path,
    cwd: &Path,
    name: &str,
    env: &[(String, String)],
    argv: &[String],
    detached: bool,
    print: bool,
) -> i32 {
    let bin = claude_bin();
    let socket = tmux_socket();
    let session = super::tmux::session_name(&super::tmux::project_prefix(repo), name);
    let targv = tmux_session_argv(&session, cwd, socket.as_deref(), env, detached, &bin, argv);
    if print {
        println!("{}", print_line("tmux", &targv));
        return 0;
    }
    // A session of this name already running: this role's own is attached to, another
    // checkout's is refused naming it (air-jc2p.4).
    let ours = super::readers::resolved(cwd);
    let same = |a: &str, b: &str| super::readers::resolved(Path::new(a)) == b;
    match super::tmux::existing(&session, &ours, &super::tmux::sessions_with_paths(), same) {
        Err(e) => {
            eprintln!("air: {e}");
            return 1;
        }
        Ok(super::tmux::Existing::Ours) => return attach_existing(&session, socket, detached),
        Ok(super::tmux::Existing::None) => {}
    }
    if !detached {
        // A tty: the session opens here. Replace this process so the terminal talks to tmux.
        let mut cmd = Command::new("tmux");
        cmd.args(&targv).current_dir(cwd);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let err = cmd.exec();
            eprintln!("air worker: could not exec tmux: {err}");
            return 1;
        }
        #[cfg(not(unix))]
        {
            return match cmd.status() {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("air worker: could not run tmux: {e}");
                    1
                }
            };
        }
    }
    match Command::new("tmux").args(&targv).current_dir(cwd).status() {
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

/// The role's session is already running: with a tty, attach this terminal to it; without one,
/// say how. Never a second `claude` for the same role (air-jc2p.1: running `air coordinator`
/// twice reuses the worktree and the session).
fn attach_existing(session: &str, socket: Option<String>, detached: bool) -> i32 {
    let l = socket
        .as_deref()
        .map(|s| format!("-L {s} "))
        .unwrap_or_default();
    if detached {
        println!("tmux session {session} is already running");
        println!("attach: tmux {l}attach -t {session}");
        return 0;
    }
    let mut args: Vec<String> = Vec::new();
    if let Some(s) = socket {
        args.extend(["-L".to_string(), s]);
    }
    args.extend(["attach".into(), "-t".into(), format!("={session}")]);
    let mut cmd = Command::new("tmux");
    cmd.args(&args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        eprintln!("air: could not exec tmux: {err}");
        1
    }
    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(s) => s.code().unwrap_or(1),
            Err(e) => {
                eprintln!("air: could not run tmux: {e}");
                1
            }
        }
    }
}

fn exec_claude(repo: &Path, env: &[(String, String)], argv: &[String], print: bool) -> i32 {
    let bin = claude_bin();
    if print {
        println!("{}", print_env_line(env, &bin, argv));
        return 0;
    }
    let mut cmd = Command::new(&bin);
    // On the process, not only in argv: nothing on a command line can clobber it (air-9dg).
    cmd.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .args(argv)
        .current_dir(repo);
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

/// Worker argv with an initial prompt in front. tmux is Air's on both paths since air-8gj
/// (`claude --tmux` required `--worktree`, which is gone), so nothing is appended after the
/// base; `AIR_TMUX_MODE` and the iTerm2 native pane went with the flag.
///
/// `prompt` is [`task_prompt`] (the sentence naming the task file), never the task itself.
///
/// The prompt goes *first*. `--disallowed-tools` takes space-separated values (cli-reference,
/// https://code.claude.com/docs/en/cli-reference, accessed 2026-08-22: example
/// `"Bash(git log *)" "Bash(git diff *)" "Edit"`), so a positional appended after the deny
/// list is read as one more deny rule, not as the prompt (air-2ct: the adopter 2026-08-22,
/// three workers idle at an empty prompt once `--tmux`, the only thing terminating the
/// list, was stripped). The reference shows the prompt positional before flags
/// (`claude -p "query" --output-format json`).
pub fn worker_argv_prompt(base: Vec<String>, prompt: Option<&str>) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(t) = prompt.filter(|t| !t.trim().is_empty()) {
        v.push(t.to_string());
    }
    v.extend(base);
    v
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
    if RESERVED.contains(&name) || name.contains('/') {
        eprintln!(
            "air worker: name must be a worktree name, no slashes, and not one of {}: those \
             read as another role",
            RESERVED.join(", ")
        );
        return 1;
    }
    launch_worker_like(repo, "worker", name, extra, tmux, task, print)
}

/// `air lane [<name>]`: the verification lane, a worker session with `AIR_ROLE=lane` and the
/// worker deny list minus `air land` (air-jc2p.2). The name defaults to `lane`; a repo whose
/// lane already has a worktree under another name (an adopter's `w4`) passes it.
pub fn lane(
    repo: &Path,
    name: Option<&str>,
    extra: &[String],
    tmux: bool,
    task: Option<&str>,
    print: bool,
) -> i32 {
    let name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("lane");
    if matches!(name, "main" | "coordinator") || name.contains('/') {
        eprintln!("air lane: name must be a worktree name, no slashes, not main or coordinator");
        return 1;
    }
    launch_worker_like(repo, "lane", name, extra, tmux, task, print)
}

/// Create or reuse `.claude/worktrees/<name>` (air-fdz). Not under `--print`, which runs
/// nothing. `who` prefixes every line.
fn ensure_worktree(repo: &Path, name: &str, who: &str) -> Result<(), ()> {
    match super::worktree::ensure(&super::worktree::main_checkout(repo), name) {
        Ok(c) => {
            if !c.existed {
                eprintln!("{who}: created {} on {}", c.path.display(), c.branch);
            }
            if !c.copied.copied.is_empty() {
                eprintln!(
                    "{who}: copied {} file(s) from .worktreeinclude",
                    c.copied.copied.len()
                );
            }
            for s in c.copied.skipped.iter().chain(c.copied.unsupported.iter()) {
                eprintln!("{who}: .worktreeinclude: skipped {s}");
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("{who}: {e}");
            Err(())
        }
    }
}

fn launch_worker_like(
    repo: &Path,
    role: &str,
    name: &str,
    extra: &[String],
    tmux: bool,
    task: Option<&str>,
    print: bool,
) -> i32 {
    let who = format!("air {role}");
    let roles = match roles_file(repo) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{who}: {e}");
            return 1;
        }
    };
    // The worktree is Air's to create and fill (air-fdz); claude is handed the existing one
    // by name and keeps its isolation.
    if !print && ensure_worktree(repo, name, &who).is_err() {
        return 1;
    }
    let env = role_env(role, name, &super::tmux::project_prefix(repo));
    let argv = match worker_argv_for(repo, role, name, &roles, extra) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{who}: {e}");
            return 1;
        }
    };
    // claude runs IN the worktree (air-8gj): the cwd is the isolation now, with the hook.
    let wt = super::worktree::dir_for(&super::worktree::main_checkout(repo), name);
    if !(tmux || task.is_some()) {
        return exec_claude(&wt, &env, &argv, print);
    }
    // The task goes to a file; argv gets a fixed sentence naming it (air-er0). Written under
    // `--print` too, so the printed command is one that runs.
    let prompt = match task.filter(|t| !t.trim().is_empty()) {
        Some(t) => match task_file(repo, name, t) {
            Ok(p) => Some(task_prompt(&p)),
            Err(e) => {
                eprintln!("{who}: {e}");
                return 1;
            }
        },
        None => None,
    };
    // The prompt goes first (air-2ct: after the deny list it reads as one more deny rule).
    let argv = worker_argv_prompt(argv, prompt.as_deref());
    let detached = launch_mode(std::io::stdin().is_terminal(), true) == Launch::Detached;
    spawn_tmux(repo, &wt, name, &env, &argv, detached, print)
}

pub fn coordinator(repo: &Path, extra: &[String], print: bool) -> i32 {
    // Metis first, because its split paragraph goes into the file the next line writes
    // (air-g5o). Every note is printed and nothing refuses: a missing planning tool must not
    // cost the owner their coordinator session.
    let cfg = super::metis::config(repo);
    let (metis_argv, notes) = super::metis::attach(&cfg, super::metis::on_path());
    for note in notes {
        eprintln!("{note}");
    }
    // Keyed on what was ATTACHED, not on what the config asked for: the split opens "Metis is
    // attached to this session", and a session that got nothing must not be told it did.
    let roles = match coordinator_roles_file(repo, !metis_argv.is_empty()) {
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
    let argv = match coordinator_argv_for(repo, &roles, &flag, &metis_argv, extra) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air coordinator: {e}");
            return 1;
        }
    };
    let env = coordinator_env(&super::tmux::project_prefix(repo));
    // Its own worktree, like every other role (owner, 2026-09-14 and 2026-09-25; air-jc2p.1).
    // The failures: a coordinator's commit in the main checkout invalidated four workers'
    // landability at an adopter on 2026-09-06, and a verify run there dirtied it and moved main
    // under a worker's green here the same day. Its commits now reach main in the lane's batch.
    // Always in tmux, so the owner attaches to it the way they attach to every worker; a second
    // `air coordinator` attaches to the running one rather than starting another.
    let name = "coordinator";
    if !print && ensure_worktree(repo, name, "air coordinator").is_err() {
        return 1;
    }
    let wt = super::worktree::dir_for(&super::worktree::main_checkout(repo), name);
    let detached = launch_mode(std::io::stdin().is_terminal(), true) == Launch::Detached;
    spawn_tmux(repo, &wt, name, &env, &argv, detached, print)
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
        // air-8gj: no `--worktree`; the cwd is the worktree and the fence hook holds the line.
        assert!(!v.contains(&"--worktree".to_string()));
        assert_eq!(v[0], "--append-system-prompt-file");
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
        // air-jc2p.4: the harness's session name is the tmux session's, project and role.
        assert!(
            v.windows(2)
                .any(|w| w[0] == "--name" && w[1] == "air-frontend")
        );
        let i = v.iter().position(|a| a == "--disallowed-tools").unwrap();
        assert_eq!(&v[i + 1..i + 1 + WORKER_DENY.len()], WORKER_DENY);
        assert_eq!(&v[v.len() - 2..], ["--model", "x"]);
    }

    /// air-jc2p.2: the lane is a worker with `air land` allowed, and nothing else changed.
    #[test]
    fn lane_argv_is_a_workers_with_land_allowed() {
        let v = lane_argv("lane", "air", Path::new("/r/.air/roles.md"), &[]);
        let i = v.iter().position(|a| a == "--settings").unwrap();
        let settings: serde_json::Value = serde_json::from_str(&v[i + 1]).unwrap();
        assert_eq!(settings["env"]["AIR_ROLE"], "lane");
        assert_eq!(settings["env"]["AIR_ENFORCE"], "1");
        assert!(v.windows(2).any(|w| w[0] == "--name" && w[1] == "air-lane"));
        assert!(!v.contains(&"Bash(air land *)".to_string()));
        for d in WORKER_DENY.iter().filter(|d| **d != "Bash(air land *)") {
            assert!(v.contains(&(*d).to_string()), "{d}");
        }
        // And a worker cannot start one.
        assert!(WORKER_DENY.contains(&"Bash(air lane *)"));
    }

    /// The prompt must come before `--disallowed-tools`, whose values are space-separated
    /// (cli-reference, accessed 2026-08-22); after it, claude reads the task as a deny rule.
    #[test]
    fn task_precedes_the_deny_list_and_nothing_follows_it() {
        let base = worker_argv("w", "air", Path::new("/r/roles.md"), &[]);
        let v = worker_argv_prompt(base.clone(), Some("fix zz-1 end to end"));
        assert_eq!(v[0], "fix zz-1 end to end");
        let deny = v.iter().position(|a| a == "--disallowed-tools").unwrap();
        assert!(deny > 0, "task must not follow the variadic deny list");
        assert_eq!(&v[1..], &base[..]);
        // air-8gj: `--tmux` is never passed; tmux is Air's on both paths.
        assert!(!v.iter().any(|a| a.starts_with("--tmux")));
        let v = worker_argv_prompt(base.clone(), None);
        assert_eq!(v, base);
        assert_eq!(v[0], "--append-system-prompt-file");
    }

    /// air-er0: the prompt names the file and carries none of the task.
    #[test]
    fn task_prompt_carries_the_path_and_none_of_the_task() {
        let p = task_prompt(Path::new("/r/.air/tasks/w1.md"));
        assert_eq!(
            p,
            "Your task is in /r/.air/tasks/w1.md. Read that file and carry it out."
        );
        assert!(task_is_prompt(&worker_argv_prompt(vec![], Some(&p)), &p));
    }

    /// `--print` pasted into `sh -c` must reproduce the exec argv for a task with a space,
    /// a single quote, and a `$`.
    #[test]
    fn print_line_round_trips_through_sh() {
        let task = "fix it's $HOME \"now\"";
        let argv = worker_argv_prompt(
            worker_argv("w", "air", Path::new("/r/roles.md"), &[]),
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
    fn tmux_argv_opens_in_the_worktree_carries_env_by_dash_e_and_detaches_only_when_asked() {
        let argv = vec!["do x".to_string()];
        let env = vec![("AIR_ENFORCE".to_string(), "1".to_string())];
        let wt = Path::new("/r/.claude/worktrees/w");
        let v = tmux_session_argv("w", wt, Some("air-test"), &env, true, "claude", &argv);
        assert_eq!(
            v,
            [
                "-L",
                "air-test",
                "new-session",
                "-d",
                "-s",
                "w",
                "-e",
                "AIR_ENFORCE=1",
                "-c",
                "/r/.claude/worktrees/w",
                "--",
                "claude",
                "do x"
            ]
        );
        assert!(
            !v.iter()
                .any(|a| a.starts_with("--tmux") || a == "--worktree")
        );
        // A tty: the same session, attached (no -d), which is what `claude --tmux` used to be.
        let v = tmux_session_argv("w", wt, None, &[], false, "claude", &argv);
        assert_eq!(v[0], "new-session");
        assert!(!v.iter().any(|a| a == "-d" || a == "-e"));
    }

    /// air-9dg: a pass-through `--settings` is merged into Air's, never a second flag; a file
    /// path is refused naming what it would drop; Air's env values win inside `env`.
    #[test]
    fn passthrough_settings_merge_into_one_blob_and_a_file_is_refused() {
        let extra: Vec<String> = [
            "--model",
            "x",
            "--settings",
            r#"{"remoteControlAtStartup":false,"env":{"FOO":"bar","AIR_ENFORCE":"0"}}"#,
        ]
        .map(String::from)
        .to_vec();
        let (rest, theirs) = split_settings(&extra).unwrap();
        assert_eq!(rest, ["--model", "x"]);
        assert_eq!(theirs.len(), 1);
        let mut argv = worker_argv("w", "air", Path::new("/r/roles.md"), &[]);
        merge_settings(&mut argv, &theirs);
        assert_eq!(argv.iter().filter(|a| *a == "--settings").count(), 1);
        let i = argv.iter().position(|a| a == "--settings").unwrap();
        let s: serde_json::Value = serde_json::from_str(&argv[i + 1]).unwrap();
        assert_eq!(s["remoteControlAtStartup"], false);
        assert_eq!(s["env"]["FOO"], "bar");
        assert_eq!(s["env"]["AIR_ENFORCE"], "1");
        assert_eq!(s["env"]["BEADS_ACTOR"], "w");
        // `--settings=<json>` is the same thing.
        let (rest, theirs) = split_settings(&[r#"--settings={"a":1}"#.to_string()]).unwrap();
        assert!(rest.is_empty());
        assert_eq!(theirs[0]["a"], 1);
        // A file path cannot be merged; it is refused with the four names in the message.
        let err =
            split_settings(&["--settings".to_string(), "/tmp/s.json".to_string()]).unwrap_err();
        assert!(
            err.contains("AIR_ENFORCE") && err.contains("/tmp/s.json"),
            "{err}"
        );
        // And a value-less flag is an error rather than a silently eaten argument.
        assert!(split_settings(&["--settings".to_string()]).is_err());
    }

    /// air-9dg: the printed line carries the env as shell assignments in front of the exec.
    #[test]
    fn print_env_line_prefixes_assignments() {
        let env = role_env("worker", "w1", "air");
        let line = print_env_line(&env, "claude", &["--model".into(), "x".into()]);
        assert_eq!(
            line,
            "AIR_ROLE=worker BEADS_ACTOR=w1 AIR_ENFORCE=1 AIR_PROJECT=air claude --model x"
        );
    }

    #[test]
    fn coordinator_argv_attaches_the_channel() {
        let v = coordinator_argv("air", Path::new("/r/.air/roles.md"), "--channels", &[], &[]);
        assert_eq!(&v[..2], ["--channels", "server:air"]);
        assert!(
            v.windows(2)
                .any(|w| w[0] == "--name" && w[1] == "air-coordinator")
        );
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
