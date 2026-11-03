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
//! Every run, local or not, is one row in the ledger's `runs` table (air-opq.3): session,
//! commit, tree, command, exit, times, output size, the target, and the machine as one probe
//! command read it there before the run (hostname, `AIR_IMAGE` if set, `uname -m`, `nproc`,
//! the CPU model). `--json` prints the row after the command's output. A run is not a verify:
//! no green query reads the table and the close gate never looks at it.
//!
//! Recorded failure: an adopter's workers ran Linux-only and heavy commands on the Mac slowly
//! or not at all, or reached a pod by hand outside Air, so no record said which machine a
//! number came from (adopter, 2026-10-29/30; epic air-opq). Removed when the harness or the
//! repo's own tooling runs a command on a named remote and records the machine and the commit.
//!
//! `--pull <glob>` (repeatable) brings declared result paths back (air-opq.4): after the
//! command exits, the paths under the target's checkout that match come back as one tar
//! stream over the same ssh and are unpacked into the calling worktree at the same relative
//! paths, overwriting. One stream rather than a copy per path, because a per-path copy is not
//! incremental and drops ownership and permissions (an adopter's survey of `kubectl cp`).
//! Nothing comes back without `--pull`, a path outside the checkout is never listed, and a glob
//! that matches nothing is reported and is not an error. The row names what came back.

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use air_ledger::runs::{Machine, Run};

use crate::cmd::{now, open};
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
    pub fn dest(&self) -> String {
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

pub fn run(repo: &Path, command: &[String], pull: &[String], json: bool) -> i32 {
    let Some((prog, rest)) = command.split_first() else {
        eprintln!("air run: no command given, e.g. `air run -- make test`");
        return 2;
    };
    let (ledger, session) = match open(repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("air run: {e}");
            return 1;
        }
    };
    let target = match super::handover::air_json(repo) {
        Some(cfg) => target_for(&cfg, &session),
        None => Ok(None),
    };
    let sha = match git::head(repo) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("air run: {e}");
            return 1;
        }
    };
    let mut row = Run {
        id: air_ledger::verify::new_id(),
        session: session.clone(),
        tree: super::green::tree_of(repo, &sha).ok(),
        sha,
        command: command.join(" "),
        exit_code: 0,
        started_at: String::new(),
        finished_at: String::new(),
        duration_ms: 0,
        output_bytes: 0,
        target: "local".into(),
        machine: Machine::default(),
        pulled: vec![],
        pulled_bytes: 0,
    };
    let mut pulled = pull::Pulled::default();
    let (exit, bytes) = match target {
        Ok(None) => {
            row.machine = probe(Command::new("sh").arg("-c").arg(PROBE));
            timed(&mut row, || local(repo, prog, rest))
        }
        Ok(Some(t)) => {
            row.target = format!("{}:{}", t.dest(), t.path);
            row.machine = probe(Command::new("ssh").arg(t.dest()).arg(PROBE));
            let sha = row.sha.clone();
            timed(&mut row, || {
                remote(repo, &session, &sha, &t, command, pull, &mut pulled)
            })
        }
        Err(why) => {
            eprintln!("air run: .claude/air.json: {why}");
            return 2;
        }
    };
    row.exit_code = exit;
    row.output_bytes = bytes;
    row.pulled = pulled.paths;
    row.pulled_bytes = pulled.bytes;
    if let Err(e) = ledger.record_run(&row) {
        eprintln!("air run: ledger: {e}");
    }
    if json {
        match serde_json::to_string_pretty(&row) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("air run: json error: {e}"),
        }
    }
    exit
}

/// The machine, read there by one shell command before the run: hostname, `AIR_IMAGE` (empty
/// when the target sets none), `uname -m`, the processor count, the CPU model. One line each,
/// Linux or macOS.
const PROBE: &str = r#"printf '%s
' "$(hostname)" "${AIR_IMAGE:-}" "$(uname -m)" "$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null)" "$(sed -n 's/^model name[^:]*: *//p' /proc/cpuinfo 2>/dev/null | head -1; sysctl -n machdep.cpu.brand_string 2>/dev/null)""#;

/// Run the probe and read its five lines; a probe that fails leaves the fields empty.
fn probe(cmd: &mut Command) -> Machine {
    let out = cmd
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let mut lines = out.lines().map(|l| l.trim().to_string());
    let hostname = lines.next().unwrap_or_default();
    let image = lines.next().filter(|s| !s.is_empty());
    let arch = lines.next().unwrap_or_default();
    let nproc = lines.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cpu = lines.next().unwrap_or_default();
    Machine {
        hostname,
        image,
        arch,
        nproc,
        cpu,
    }
}

/// Stamp the row's times and duration around `f`.
fn timed(row: &mut Run, f: impl FnOnce() -> (i32, i64)) -> (i32, i64) {
    let t0 = std::time::Instant::now();
    row.started_at = now();
    let r = f();
    row.finished_at = now();
    row.duration_ms = i64::try_from(t0.elapsed().as_millis()).unwrap_or(i64::MAX);
    r
}

/// No target: the command, here, its output mirrored to ours and counted.
fn local(repo: &Path, prog: &str, args: &[String]) -> (i32, i64) {
    match super::record::run_tee(prog, args, repo) {
        Ok((code, bytes, _)) => (code, bytes),
        Err(e) => {
            eprintln!("air run: {prog}: {e}");
            (127, 0)
        }
    }
}

fn remote(
    repo: &Path,
    session: &str,
    sha: &str,
    t: &Target,
    command: &[String],
    pull: &[String],
    pulled: &mut pull::Pulled,
) -> (i32, i64) {
    let dest = t.dest();
    let path = sh_quote(&t.path);
    if let Some(code) = prepare_target(repo, session, sha, t) {
        return (code, 0);
    }
    // Through the same pump as `air record`, so the output streams as it arrives and is
    // counted.
    let args = remote_args(t, command);
    match super::record::run_tee("ssh", &args, repo) {
        Ok((code, bytes, _)) => {
            // After the command, whatever its exit: a failed benchmark's partial results are
            // still the results asked for.
            if !pull.is_empty() {
                *pulled = pull::fetch(repo, &dest, &path, pull);
            }
            (code, bytes)
        }
        Err(e) => {
            eprintln!("air run: ssh: {e}");
            (127, 0)
        }
    }
}

/// `--pull`: declared result paths back from the target as one tar stream (air-opq.4).
pub mod pull {
    use std::path::Path;
    use std::process::{Command, Stdio};

    use super::sh_quote;

    /// What came back: the paths, relative to the worktree, and the bytes under them there.
    #[derive(Debug, Default, Clone, PartialEq, Eq)]
    pub struct Pulled {
        pub paths: Vec<String>,
        pub bytes: i64,
    }

    /// Expands each glob in the target's checkout and prints one line per match, `+<path>`,
    /// or `-<glob>` for a glob with no match. Runs as `sh -c '<script>' air <globs…>`, so the
    /// globs arrive as arguments and expand by the shell's own rules, never as code. A match
    /// outside the checkout (absolute, or with a `..` component) is dropped here, and the
    /// local `tar` refuses such a member as a second guard.
    const LIST: &str = r#"for g in "$@"; do
  n=0
  for p in $g; do
    [ -e "$p" ] || continue
    case "$p" in /*|..|../*|*/..|*/../*) continue ;; esac
    printf '+%s
' "$p"
    n=1
  done
  [ "$n" = 1 ] || printf -- '-%s
' "$g"
done"#;

    /// Parse the list script's output: the paths matched, and the globs that matched nothing.
    pub fn parse_list(out: &str) -> (Vec<String>, Vec<String>) {
        let mut paths = Vec::new();
        let mut unmatched = Vec::new();
        for line in out.lines() {
            if let Some(p) = line.strip_prefix('+') {
                if !p.is_empty() && !paths.iter().any(|q| q == p) {
                    paths.push(p.to_string());
                }
            } else if let Some(g) = line.strip_prefix('-') {
                unmatched.push(g.to_string());
            }
        }
        (paths, unmatched)
    }

    /// List, then stream: `ssh <dest> "cd <path> && sh -c LIST air <globs>"`, and when
    /// anything matched, `ssh <dest> "cd <path> && tar -cf - -- <paths>"` piped into
    /// `tar -xf - -C <repo>`. Reports each glob that matched nothing and one line for what
    /// came back; a transport failure is reported and leaves the run's exit code alone.
    pub fn fetch(repo: &Path, dest: &str, path: &str, globs: &[String]) -> Pulled {
        let args = globs
            .iter()
            .map(|g| sh_quote(g))
            .collect::<Vec<_>>()
            .join(" ");
        let list = format!("cd {path} && sh -c {} air {args}", sh_quote(LIST));
        let out = Command::new("ssh")
            .arg(dest)
            .arg(list)
            .stdin(Stdio::null())
            .output();
        let out = match out {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            Ok(o) => {
                eprintln!(
                    "air run: --pull: listing on the target failed (exit {}); nothing pulled",
                    o.status.code().unwrap_or(1)
                );
                return Pulled::default();
            }
            Err(e) => {
                eprintln!("air run: --pull: ssh: {e}; nothing pulled");
                return Pulled::default();
            }
        };
        let (paths, unmatched) = parse_list(&out);
        for g in &unmatched {
            eprintln!("air run: --pull {g}: matched nothing on the target");
        }
        if paths.is_empty() {
            return Pulled::default();
        }
        let quoted = paths
            .iter()
            .map(|p| sh_quote(p))
            .collect::<Vec<_>>()
            .join(" ");
        let mut send = match Command::new("ssh")
            .arg(dest)
            .arg(format!("cd {path} && tar -cf - -- {quoted}"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("air run: --pull: ssh: {e}; nothing pulled");
                return Pulled::default();
            }
        };
        let Some(stream) = send.stdout.take() else {
            eprintln!("air run: --pull: no stream from the target; nothing pulled");
            return Pulled::default();
        };
        let unpack = Command::new("tar")
            .arg("-xf")
            .arg("-")
            .arg("-C")
            .arg(repo)
            .stdin(Stdio::from(stream))
            .status();
        let sent = send.wait().map(|s| s.success()).unwrap_or(false);
        match unpack {
            Ok(s) if s.success() && sent => {}
            Ok(s) => {
                eprintln!(
                    "air run: --pull: the tar stream failed (target {}, unpack exit {}); the \
                     worktree may hold part of it",
                    if sent { "ok" } else { "failed" },
                    s.code().unwrap_or(1)
                );
                return Pulled::default();
            }
            Err(e) => {
                eprintln!("air run: --pull: tar: {e}; nothing pulled");
                return Pulled::default();
            }
        }
        let bytes = paths.iter().map(|p| size_of(&repo.join(p))).sum();
        eprintln!("air run: pulled {} ({bytes} bytes)", paths.join(", "));
        Pulled { paths, bytes }
    }

    /// Bytes of a file, or of every file under a directory; 0 for what cannot be read.
    pub fn size_of(p: &Path) -> i64 {
        let Ok(meta) = std::fs::symlink_metadata(p) else {
            return 0;
        };
        if meta.is_dir() {
            std::fs::read_dir(p)
                .map(|d| d.flatten().map(|e| size_of(&e.path())).sum())
                .unwrap_or(0)
        } else {
            i64::try_from(meta.len()).unwrap_or(i64::MAX)
        }
    }

    #[cfg(test)]
    mod tests {
        #![allow(clippy::unwrap_used)]
        use super::*;

        #[test]
        fn list_output_splits_matches_from_misses_and_dedups() {
            let (paths, missed) =
                parse_list("+results/a.txt\n-nothing/*\n+results/a.txt\n+out\n\n");
            assert_eq!(paths, vec!["results/a.txt", "out"]);
            assert_eq!(missed, vec!["nothing/*"]);
        }

        #[test]
        fn the_list_script_drops_paths_outside_the_checkout() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join("results")).unwrap();
            std::fs::write(dir.path().join("results/a.txt"), "x").unwrap();
            std::fs::write(dir.path().join("top"), "y").unwrap();
            let out = Command::new("sh")
                .current_dir(dir.path())
                .args([
                    "-c",
                    LIST,
                    "air",
                    "results/*",
                    "../*",
                    "/etc/passwd",
                    "top",
                    "none/*",
                ])
                .output()
                .unwrap();
            let (paths, missed) = parse_list(&String::from_utf8_lossy(&out.stdout));
            assert_eq!(paths, vec!["results/a.txt", "top"]);
            assert_eq!(missed, vec!["../*", "/etc/passwd", "none/*"]);
        }

        #[test]
        fn size_of_sums_a_directory() {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join("d/e")).unwrap();
            std::fs::write(dir.path().join("d/a"), "12345").unwrap();
            std::fs::write(dir.path().join("d/e/b"), "12").unwrap();
            assert_eq!(size_of(&dir.path().join("d")), 7);
            assert_eq!(size_of(&dir.path().join("missing")), 0);
        }
    }
}

/// Put `sha` on the target: a repository at the path once, the push to the session's own ref,
/// the detached checkout. Says what it is doing on stderr. Some(exit) when a step failed, with
/// the step named; None when the target holds `sha` checked out.
pub fn prepare_target(repo: &Path, session: &str, sha: &str, t: &Target) -> Option<i32> {
    let dest = t.dest();
    let path = sh_quote(&t.path);
    let short = sha.get(..8).unwrap_or(sha);
    eprintln!("air run: {session} at {short} on {dest}:{}", t.path);
    // Once: a repository at the path. Idempotent, so every run may ask.
    let init = format!("test -d {path}/.git || {{ mkdir -p {path} && git init -q {path}; }}");
    if let Some(code) = step("init", ssh(&dest, &init)) {
        return Some(code);
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
        return Some(code);
    }
    let checkout = format!("cd {path} && git checkout -q --detach {sha}");
    step("checkout", ssh(&dest, &checkout))
}

/// The argv after `ssh` that runs `command` in the target's path: the destination and one
/// shell line.
pub fn remote_args(t: &Target, command: &[String]) -> Vec<String> {
    let cmd = command
        .iter()
        .map(|a| sh_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    vec![t.dest(), format!("cd {} && {cmd}", sh_quote(&t.path))]
}

/// The machine a command would run on: the target's, read over ssh, or this one's.
pub fn probe_machine(t: Option<&Target>) -> Machine {
    match t {
        Some(t) => probe(Command::new("ssh").arg(t.dest()).arg(PROBE)),
        None => probe(Command::new("sh").arg("-c").arg(PROBE)),
    }
}

/// `ssh <dest> <script>` for a preparatory step, stdout and stderr ours.
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
