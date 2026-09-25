//! Which processes are reading each fleet tree: every process whose working directory is
//! inside a worktree of this repo, the main checkout included (owner, 2026-09-25).
//!
//! **The failure it removes.** Air's in-flight refusal and its idle conditions see only runs
//! recorded through `air record`. An adopter built its own `make tree-readers` for three things
//! Air could not see, all on 2026-09-07: a landing about to move main under a lane 1m41s into an
//! unrecorded `fitness-prove` run; a worker under a lane running an unrecorded precheck, so
//! "mid-precheck" and "doing nothing" were the same row and the coordinator prompted a busy
//! worker; and four dev-server watchers alive for eighteen hours, holding no lease, waking to
//! ~46% CPU each whenever Rust files changed. The adopter ran it before every landing and
//! before calling anyone idle.
//!
//! **A fact, never a refusal.** `air status` prints it, `idle-without-claim` reads it, and
//! `air land` warns with it. Nothing is refused on it.
//!
//! **What is listed.** Every process with its cwd in a tree, grouped by the tree (longest path
//! wins, on a component boundary, so the main checkout does not swallow a worktree nested under
//! it and `w1` never matches `w10`). Each is marked `session` or not:
//!
//! - a Claude Code process (executable `claude`, or a pid a session row recorded), every
//!   ancestor of one (the shell or tmux pane hosting it), and every descendant reached WITHOUT
//!   passing through a shell: the harness's own helpers and MCP servers (`air mcp`,
//!   `caffeinate`, a status line). These are the session itself.
//! - anything reached through a shell is work: the Bash tool runs commands in a shell, so a
//!   backgrounded `cargo test` or a precheck is a non-session reader, which is the adopter's
//!   question ("what ELSE is running").
//!
//! This `air` process, its children (the `lsof` and `ps` it runs) and its ancestors are not
//! listed at all: the shell that ran `air status` is not a reader of anything.
//!
//! **What is shown of a process.** Its pid, how long it has run, and its executable's name,
//! truncated. Never its argv, which can carry prompt text.
//!
//! **Cost.** One `ps` and, on macOS, one `lsof -d cwd` (Linux reads `/proc/<pid>/cwd`). `lsof`
//! measured 0.7 to 1.1 s here on 2026-09-25 with ~1160 processes, so `air status` starts the
//! lookup on a thread before its git and bd work and joins it at the end; each child runs under
//! [`BUDGET`], recorded as the `tree-readers` budget. Missing, slow or failing, the answer is
//! `unknown (why)`, never an empty list.
//!
//! **Removed when** every process that runs in a fleet tree is one Air recorded (a recorded
//! precheck run kind, plan 0009 B2, and the lane's verify already are), so the ledger alone
//! answers "what is running there"; or when the harness exposes a session's background tasks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;

/// How long each child (`ps`, `lsof`) may take. `lsof` measured ~0.7-1.1 s (module doc), so 4 s
/// is roughly 4x; hitting it answers `unknown`, which the caller is told.
pub const BUDGET: Duration = Duration::from_secs(4);

/// The longest executable name shown. Names, not argv; this only keeps a line readable.
const COMMAND_WIDTH: usize = 20;

/// One process as `ps` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: i64,
    pub ppid: i64,
    pub elapsed_secs: Option<i64>,
    /// The executable's base name, truncated. Never the argv.
    pub command: String,
}

/// One process reading a tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reader {
    pub pid: i64,
    pub elapsed_secs: Option<i64>,
    pub command: String,
    /// Part of a Claude Code session (module doc), not work running beside it.
    pub session: bool,
}

/// The readers of one tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tree {
    pub worker: String,
    pub path: String,
    pub readers: Vec<Reader>,
}

/// The whole answer. `unknown` set means the lookup did not run or did not finish, and then
/// `trees` is empty and says nothing. `examined` is how many processes had a cwd to compare,
/// so "none" is distinguishable from "not looked" (decisions.md, 2026-09-06).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TreeReaders {
    pub examined: usize,
    pub trees: Vec<Tree>,
    pub unknown: Option<String>,
}

impl TreeReaders {
    pub fn unknown(why: impl Into<String>) -> Self {
        Self {
            unknown: Some(why.into()),
            ..Self::default()
        }
    }

    /// Does anything that is not a session have its cwd in `worker`'s tree?
    pub fn busy(&self, worker: &str) -> bool {
        self.trees
            .iter()
            .any(|t| t.worker == worker && t.readers.iter().any(|r| !r.session))
    }
}

/// What the OS said: every process, and the cwd of each one it would tell us.
#[derive(Debug, Clone, Default)]
pub struct Raw {
    pub procs: Vec<Proc>,
    pub cwds: Vec<(i64, String)>,
}

/// `ps -A -o pid=,ppid=,etime=,comm=`: three fields, then the executable (which may contain
/// spaces). Lines that do not parse are skipped.
pub fn parse_ps(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let pid = it.next()?.parse().ok()?;
            let ppid = it.next()?.parse().ok()?;
            let elapsed_secs = parse_etime(it.next()?);
            let comm = it.collect::<Vec<_>>().join(" ");
            Some(Proc {
                pid,
                ppid,
                elapsed_secs,
                command: short_command(&comm),
            })
        })
        .collect()
}

/// The executable's base name, a login shell's leading `-` kept, cut to [`COMMAND_WIDTH`].
pub fn short_command(comm: &str) -> String {
    let base = comm.rsplit('/').next().unwrap_or(comm);
    base.chars().take(COMMAND_WIDTH).collect()
}

/// `ps` elapsed time, `[[dd-]hh:]mm:ss`, in seconds.
pub fn parse_etime(s: &str) -> Option<i64> {
    let (days, rest) = match s.split_once('-') {
        Some((d, r)) => (d.parse::<i64>().ok()?, r),
        None => (0, s),
    };
    let parts: Vec<i64> = rest
        .split(':')
        .map(|p| p.parse::<i64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let (h, m, sec) = match parts.as_slice() {
        [m, s] => (0, *m, *s),
        [h, m, s] => (*h, *m, *s),
        _ => return None,
    };
    days.checked_mul(86_400)?
        .checked_add(h.checked_mul(3_600)?)?
        .checked_add(m.checked_mul(60)?)?
        .checked_add(sec)
}

/// `lsof -F pn` output: a `p<pid>` line opens a process, an `n<path>` line is its cwd (the
/// only descriptor asked for). `f` and anything else is ignored.
pub fn parse_lsof(text: &str) -> Vec<(i64, String)> {
    let mut out = Vec::new();
    let mut pid: Option<i64> = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.trim().parse().ok();
        } else if let Some(n) = line.strip_prefix('n')
            && let Some(p) = pid
        {
            out.push((p, n.to_string()));
        }
    }
    out
}

/// Which tree `cwd` is in: the longest tree path that equals it or is a parent of it on a
/// `/` boundary. `/r/.claude/worktrees/w1` is not inside `/r/.claude/worktrees/w10`.
pub fn tree_for<'a>(cwd: &str, trees: &'a [(String, String)]) -> Option<&'a str> {
    trees
        .iter()
        .filter(|(_, path)| {
            let path = path.trim_end_matches('/');
            cwd == path
                || cwd
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|(_, path)| path.trim_end_matches('/').len())
        .map(|(name, _)| name.as_str())
}

fn is_shell(command: &str) -> bool {
    matches!(
        command.trim_start_matches('-'),
        "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh" | "tcsh" | "csh" | "nu"
    )
}

/// The pids that are part of a Claude Code session (module doc).
pub fn session_side(procs: &[Proc], session_pids: &BTreeSet<i64>) -> BTreeSet<i64> {
    let by_pid: BTreeMap<i64, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    let mut children: BTreeMap<i64, Vec<&Proc>> = BTreeMap::new();
    for p in procs {
        children.entry(p.ppid).or_default().push(p);
    }
    let seeds: Vec<i64> = procs
        .iter()
        .filter(|p| p.command == "claude" || session_pids.contains(&p.pid))
        .map(|p| p.pid)
        .collect();
    let mut side = BTreeSet::new();
    for &seed in &seeds {
        // Up: whatever hosts the session.
        let mut cur = seed;
        while side.insert(cur) {
            match by_pid.get(&cur) {
                Some(p) if p.ppid > 1 => cur = p.ppid,
                _ => break,
            }
        }
        // Down, stopping at a shell: a shell's subtree is work somebody asked for.
        let mut stack = vec![seed];
        while let Some(pid) = stack.pop() {
            for c in children.get(&pid).map(Vec::as_slice).unwrap_or(&[]) {
                if !is_shell(&c.command) && side.insert(c.pid) {
                    stack.push(c.pid);
                }
            }
        }
    }
    side
}

/// This process, its ancestors, and its direct children: never listed. When the parent is a
/// shell, its other children too: `air status | grep readers` listed its own `grep` on the
/// first live run. Only a shell's: `air mcp`'s parent is `claude`, whose other children are
/// the Bash-tool shells doing real work.
pub fn own_family(procs: &[Proc], me: i64) -> BTreeSet<i64> {
    let by_pid: BTreeMap<i64, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    let shell_parent = by_pid
        .get(&me)
        .and_then(|p| by_pid.get(&p.ppid))
        .filter(|p| is_shell(&p.command))
        .map(|p| p.pid);
    let mut out: BTreeSet<i64> = procs
        .iter()
        .filter(|p| p.ppid == me || Some(p.ppid) == shell_parent)
        .map(|p| p.pid)
        .collect();
    let mut cur = me;
    while out.insert(cur) {
        match by_pid.get(&cur) {
            Some(p) if p.ppid > 1 => cur = p.ppid,
            _ => break,
        }
    }
    out
}

/// The pure join: group every cwd into its tree, mark session processes, drop our own.
/// `trees` is (worker, path) with paths already resolved the way the OS reports cwds.
pub fn group(
    raw: &Raw,
    trees: &[(String, String)],
    session_pids: &BTreeSet<i64>,
    me: i64,
) -> TreeReaders {
    let side = session_side(&raw.procs, session_pids);
    let mine = own_family(&raw.procs, me);
    let by_pid: BTreeMap<i64, &Proc> = raw.procs.iter().map(|p| (p.pid, p)).collect();
    let mut grouped: BTreeMap<&str, Vec<Reader>> = BTreeMap::new();
    for (pid, cwd) in &raw.cwds {
        if mine.contains(pid) {
            continue;
        }
        let Some(tree) = tree_for(cwd, trees) else {
            continue;
        };
        let p = by_pid.get(pid);
        grouped.entry(tree).or_default().push(Reader {
            pid: *pid,
            elapsed_secs: p.and_then(|p| p.elapsed_secs),
            command: p.map(|p| p.command.clone()).unwrap_or_else(|| "?".into()),
            session: side.contains(pid),
        });
    }
    TreeReaders {
        examined: raw.cwds.len(),
        trees: trees
            .iter()
            .filter_map(|(name, path)| {
                let mut readers = grouped.remove(name.as_str())?;
                // Longest-running first: the eighteen-hour watcher is the one to see.
                readers.sort_by_key(|r| std::cmp::Reverse(r.elapsed_secs.unwrap_or(0)));
                Some(Tree {
                    worker: name.clone(),
                    path: path.clone(),
                    readers,
                })
            })
            .collect(),
        unknown: None,
    }
}

/// `3m`, `18h`, `2d`, `45s`; `?` when unknown.
pub fn age(secs: Option<i64>) -> String {
    match secs {
        None => "?".into(),
        Some(s) if s < 60 => format!("{s}s"),
        Some(s) if s < 3_600 => format!("{}m", s / 60),
        Some(s) if s < 86_400 => format!("{}h", s / 3_600),
        Some(s) => format!("{}d", s / 86_400),
    }
}

/// `air status`'s lines: one per tree with a non-session reader, silent when there is none,
/// and one `unknown` line when the lookup could not answer.
pub fn status_lines(r: &TreeReaders) -> Vec<String> {
    if let Some(why) = &r.unknown {
        return vec![format!("readers: unknown ({why})")];
    }
    r.trees
        .iter()
        .filter_map(|t| {
            let work: Vec<String> = t
                .readers
                .iter()
                .filter(|x| !x.session)
                .map(|x| format!("{} {}", x.command, age(x.elapsed_secs)))
                .collect();
            (!work.is_empty()).then(|| {
                format!(
                    "readers: {}: {} ({})",
                    t.worker,
                    work.len(),
                    work.join(", ")
                )
            })
        })
        .collect()
}

/// `air land`'s warning about the main checkout, or None when nothing but sessions is there.
/// A warning, never a refusal: the landing moves files under these processes, and whether
/// that matters is the coordinator's call.
pub fn main_warning(r: &TreeReaders, main: &str) -> Option<String> {
    if let Some(why) = &r.unknown {
        return Some(format!(
            "warning: could not see what is reading the main checkout ({why}); this landing \
             moves files under whatever is."
        ));
    }
    let work: Vec<&Reader> = r
        .trees
        .iter()
        .filter(|t| t.path == main)
        .flat_map(|t| t.readers.iter())
        .filter(|x| !x.session)
        .collect();
    if work.is_empty() {
        return None;
    }
    let mut s = format!(
        "warning: {} process(es) not recorded by Air have their working directory in the main \
         checkout, and this landing moves files under them:",
        work.len()
    );
    for x in &work {
        s.push_str(&format!(
            "\n  pid {} {} ({})",
            x.pid,
            x.command,
            age(x.elapsed_secs)
        ));
    }
    s.push_str(
        "\n  a fact, not a refusal: stop one by pid (`kill <pid>`) if it should not see main move.",
    );
    Some(s)
}

// ----- the part that asks the OS -----

fn run_child(program: &str, args: &[&str]) -> Result<String, String> {
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{program} could not start: {e}"))?;
    match crate::git::wait_drained(child, BUDGET) {
        Err(e) => Err(format!("{program}: {e}")),
        Ok(None) => Err(format!(
            "{program} did not answer within {} s",
            BUDGET.as_secs()
        )),
        // lsof exits 1 when some process could not be read; its output is still the answer.
        Ok(Some((_, out, _))) => Ok(String::from_utf8_lossy(&out).into_owned()),
    }
}

/// Ask the OS. Linux reads `/proc/<pid>/cwd`; elsewhere `lsof`.
///
/// `AIR_TREE_READERS=off` skips it and answers `unknown`, saying so. This repo's
/// `.cargo/config.toml` sets it for everything cargo starts, because the suite spawns
/// `air status` dozens of times in parallel and each lookup is an `lsof` of about a second.
pub fn lookup() -> Result<Raw, String> {
    if std::env::var("AIR_TREE_READERS").is_ok_and(|v| v == "off") {
        return Err("switched off by AIR_TREE_READERS=off".into());
    }
    let t0 = std::time::Instant::now();
    let res = lookup_inner();
    air_ledger::budgets::record(
        air_ledger::budgets::TREE_READERS,
        t0.elapsed(),
        BUDGET,
        matches!(&res, Err(e) if e.contains("did not answer")),
    );
    res
}

fn ps() -> Result<Vec<Proc>, String> {
    let procs = parse_ps(&run_child("ps", &["-A", "-o", "pid=,ppid=,etime=,comm="])?);
    if procs.is_empty() {
        return Err("ps listed no processes".into());
    }
    Ok(procs)
}

/// `lsof` BEFORE `ps` so every cwd has a `ps` row unless its process has since exited;
/// [`known_only`] drops those. The other order listed processes born between the two calls
/// with no name (seen as `? ?` on the first live run).
fn lookup_inner() -> Result<Raw, String> {
    if Path::new("/proc/self/cwd").exists() {
        let procs = ps()?;
        let cwds = procs
            .iter()
            .filter_map(|p| {
                std::fs::read_link(format!("/proc/{}/cwd", p.pid))
                    .ok()
                    .map(|c| (p.pid, c.to_string_lossy().into_owned()))
            })
            .collect();
        return Ok(Raw { procs, cwds });
    }
    let cwds = parse_lsof(&run_child(
        "lsof",
        &["-n", "-P", "-w", "-b", "-a", "-d", "cwd", "-Fpn"],
    )?);
    Ok(known_only(Raw { procs: ps()?, cwds }))
}

/// Drop cwds whose process `ps` no longer lists: it exited between the two calls.
pub fn known_only(raw: Raw) -> Raw {
    let known: BTreeSet<i64> = raw.procs.iter().map(|p| p.pid).collect();
    Raw {
        cwds: raw
            .cwds
            .into_iter()
            .filter(|(pid, _)| known.contains(pid))
            .collect(),
        procs: raw.procs,
    }
}

/// A tree path as the OS reports cwds (`/tmp` is `/private/tmp` on macOS).
pub fn resolved(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .into_owned()
}

/// The whole thing for a caller that has the trees already: ask, then join.
pub fn gather(
    raw: Result<Raw, String>,
    trees: &[(String, PathBuf)],
    session_pids: &BTreeSet<i64>,
) -> TreeReaders {
    let trees: Vec<(String, String)> = trees
        .iter()
        .map(|(n, p)| (n.clone(), resolved(p)))
        .collect();
    match raw {
        Ok(raw) => group(&raw, &trees, session_pids, i64::from(std::process::id())),
        Err(why) => TreeReaders::unknown(why),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn p(pid: i64, ppid: i64, secs: i64, command: &str) -> Proc {
        Proc {
            pid,
            ppid,
            elapsed_secs: Some(secs),
            command: command.into(),
        }
    }

    #[test]
    fn lsof_and_ps_output_parse() {
        let lsof = "p206\nfcwd\nn/\np455\nfcwd\nn/r/.claude/worktrees/w1\np9\nfcwd\n";
        assert_eq!(
            parse_lsof(lsof),
            vec![(206, "/".into()), (455, "/r/.claude/worktrees/w1".into())]
        );
        let ps = "  74590 74003 02:05:29 /usr/local/bin/claude\n 5 1 1-02:00:00 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome Helper Renderer\nbad line\n";
        let procs = parse_ps(ps);
        assert_eq!(procs.len(), 2);
        assert_eq!(procs[0], p(74590, 74003, 7529, "claude"));
        assert_eq!(procs[1].elapsed_secs, Some(93_600));
        assert_eq!(procs[1].command.chars().count(), COMMAND_WIDTH);
        assert_eq!(parse_etime("00:07"), Some(7));
        assert_eq!(parse_etime("x"), None);
        let kept = known_only(Raw {
            procs: vec![p(206, 1, 5, "launchd")],
            cwds: parse_lsof(lsof),
        });
        assert_eq!(
            kept.cwds,
            vec![(206, "/".to_string())],
            "455 exited before ps"
        );
    }

    #[test]
    fn a_cwd_goes_to_the_longest_tree_on_a_component_boundary() {
        let trees = vec![
            ("main".to_string(), "/r".to_string()),
            ("w1".to_string(), "/r/.claude/worktrees/w1".to_string()),
            ("w10".to_string(), "/r/.claude/worktrees/w10".to_string()),
        ];
        assert_eq!(
            tree_for("/r/.claude/worktrees/w10/src", &trees),
            Some("w10")
        );
        assert_eq!(tree_for("/r/.claude/worktrees/w1", &trees), Some("w1"));
        assert_eq!(tree_for("/r/src", &trees), Some("main"));
        assert_eq!(tree_for("/rx", &trees), None);
    }

    #[test]
    fn sessions_are_marked_work_is_not_and_our_own_family_is_dropped() {
        let raw = Raw {
            procs: vec![
                p(10, 1, 100, "-zsh"),    // pane shell hosting the session
                p(11, 10, 100, "claude"), // the session
                p(12, 11, 100, "air"),    // its MCP server
                p(13, 11, 60, "zsh"),     // Bash tool shell
                p(14, 13, 180, "cargo"),  // a precheck under it
                p(20, 1, 64_800, "node"), // an orphaned watcher
                p(15, 11, 1, "zsh"),      // another Bash tool shell, running:
                p(30, 15, 1, "air"),      // this process, `air status | grep`
                p(31, 30, 1, "lsof"),     // our own child
                p(32, 15, 1, "grep"),     // our pipeline peer
            ],
            cwds: vec![
                (10, "/r/.claude/worktrees/w2".into()),
                (11, "/r/.claude/worktrees/w2".into()),
                (12, "/r/.claude/worktrees/w2".into()),
                (14, "/r/.claude/worktrees/w2/crates".into()),
                (20, "/r".into()),
                (30, "/r".into()),
                (31, "/r".into()),
                (32, "/r".into()),
                (40, "/elsewhere".into()),
            ],
        };
        let trees = vec![
            ("main".to_string(), "/r".to_string()),
            ("w2".to_string(), "/r/.claude/worktrees/w2".to_string()),
        ];
        let r = group(&raw, &trees, &BTreeSet::new(), 30);
        assert_eq!(r.examined, 9);
        assert!(r.busy("w2"));
        assert!(r.busy("main"));
        let w2 = r.trees.iter().find(|t| t.worker == "w2").unwrap();
        let work: Vec<i64> = w2
            .readers
            .iter()
            .filter(|x| !x.session)
            .map(|x| x.pid)
            .collect();
        assert_eq!(work, vec![14]);
        let main = r.trees.iter().find(|t| t.worker == "main").unwrap();
        assert_eq!(
            main.readers.len(),
            1,
            "air itself, its lsof and its pipeline peer are not readers"
        );
        assert_eq!(
            status_lines(&r),
            vec!["readers: main: 1 (node 18h)", "readers: w2: 1 (cargo 3m)"]
        );
        let warn = main_warning(&r, "/r").unwrap();
        assert!(warn.contains("pid 20 node (18h)"), "{warn}");
        assert!(main_warning(&r, "/nowhere").is_none());
    }

    #[test]
    fn unknown_is_said_and_never_reads_as_none() {
        let r = TreeReaders::unknown("lsof could not start: not found");
        assert_eq!(
            status_lines(&r),
            vec!["readers: unknown (lsof could not start: not found)"]
        );
        assert!(!r.busy("w1"));
        assert!(main_warning(&r, "/r").unwrap().contains("could not see"));
        assert!(status_lines(&TreeReaders::default()).is_empty());
    }
}
