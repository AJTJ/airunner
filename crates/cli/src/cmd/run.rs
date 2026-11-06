//! `air run -- <command…>`: run a command on the calling session's target, or locally.
//!
//! A target is a machine the session owns, named in `.claude/air.json` under `"targets"`,
//! keyed by session name. It is an argv prefix that runs a command on the machine with stdin
//! and stdout attached, and the repository directory there:
//!
//! ```json
//! {"targets": {"worker-1": {"exec": ["kubectl", "exec", "-i", "<pod>", "-c", "<c>", "-n", "ssh", "--"],
//!                           "path": "~/air/proj"}}}
//! ```
//!
//! `"kind": "ssh"` with `host`, `user` and `path` is the short form and expands to the prefix
//! `["ssh", "user@host"]`. It is a kind of its own and not an `exec` spelling because ssh hands
//! its arguments to a remote shell as one line, where `kubectl exec` passes them as argv; the
//! short form quotes the line once more for that shell, and an `exec` prefix gets it verbatim
//! (air-tfi). Every step Air runs on the machine is the prefix followed by `sh -c <line>`:
//! the probe, the repository's init, the checkout, the command, and `--pull`'s listing and
//! tar stream.
//!
//! A session with no target runs the command here, in the worktree, with the same exit code
//! and nothing added to its output. With a target Air initialises a repository at `path` on
//! the machine the first time, pushes HEAD to `refs/air/<session>` there (force: the ref is the
//! session's own), checks that commit out detached, and runs the command in `path` with its
//! output streamed back as it arrives. The exit code is the command's.
//!
//! The push goes through the same prefix, as git's `ext::` remote helper: the prefix, then
//! `git-receive-pack <path>`, speaking the git protocol over the prefix's stdin and stdout
//! (`git help remote-ext`; the helper splits its line on spaces, with `% ` a literal space
//! and `%%` a percent, and runs the words as argv with no shell). `ext` is off by default, so
//! the push passes `-c protocol.ext.allow=always` and `.git/config` is not changed. It goes to
//! the configured target only, never to a remote named in `.git/config`, so the rule that
//! nothing leaves the machine without the owner's word is untouched. The prefix's program is
//! resolved on `PATH`, which is also how a test stands in for it.
//!
//! Recorded failure behind the prefix: the adopter whose pods this runs on reaches them only
//! with `kubectl exec`, and their per-user ssh host serves sftp alone, so no target could name
//! a pod and the runs stayed outside Air (adopter, 2026-10-30; air-tfi). Removed when the
//! harness or git carries a configurable transport Air can read, or when a round of `runs`
//! rows at every adopter shows only ssh targets.
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
//! stream over the same prefix and are unpacked into the calling worktree at the same relative
//! paths, overwriting. One stream rather than a copy per path, because a per-path copy is not
//! incremental and drops ownership and permissions (an adopter's survey of `kubectl cp`).
//! Nothing comes back without `--pull`, a path outside the checkout is never listed, and a glob
//! that matches nothing is reported and is not an error. The row names what came back.

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use air_ledger::runs::{Machine, Run};

use crate::cmd::{now, open};
use crate::git;

/// One target from `"targets"`: how to run a command on the machine, and where the
/// repository is there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The argv prefix that runs a command on the machine with stdin and stdout attached:
    /// `["ssh", "me@box"]`, or `["kubectl", "exec", "-i", "<pod>", "--"]`. Never empty.
    pub exec: Vec<String>,
    /// Whether the prefix hands its arguments to a remote shell as one line (ssh does) rather
    /// than as argv (`kubectl exec` does). The line Air appends is then quoted once more.
    pub shell: bool,
    /// What messages and the row call the machine: `user@host` for the ssh short form, the
    /// prefix joined by spaces for `exec`.
    pub label: String,
    /// The repository directory on the machine. A leading `~` is git's and the remote shell's
    /// to expand.
    pub path: String,
}

impl Target {
    /// The machine's name in messages and the row: `user@host`, or the prefix.
    pub fn dest(&self) -> String {
        self.label.clone()
    }

    /// The program and arguments that run one shell line on the machine: the prefix, then
    /// `sh -c <line>`, the line quoted for the remote shell where the prefix has one.
    pub fn command_line(&self, line: &str) -> (String, Vec<String>) {
        let mut args: Vec<String> = self.exec.iter().skip(1).cloned().collect();
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(if self.shell {
            sh_quote(line)
        } else {
            line.to_string()
        });
        let prog = self
            .exec
            .first()
            .cloned()
            .unwrap_or_else(|| "sh".to_string());
        (prog, args)
    }

    /// [`Self::command_line`] as a `Command`, stdin closed.
    pub fn command(&self, line: &str) -> Command {
        let (prog, args) = self.command_line(line);
        let mut cmd = Command::new(prog);
        cmd.args(args).stdin(Stdio::null());
        cmd
    }

    /// The URL the push goes to: git's `ext::` helper runs the prefix with `git-receive-pack
    /// <path>` after it and speaks the protocol over its stdin and stdout. Words are separated
    /// by spaces, so a space in a word is `% ` and a percent `%%`; the helper runs the words
    /// as argv with no shell between (checked against git 2.54.0, air-tfi).
    pub fn push_url(&self) -> String {
        let path = if self.shell {
            sh_quote(&self.path)
        } else {
            self.path.clone()
        };
        let words = self
            .exec
            .iter()
            .map(|w| ext_word(w))
            .chain(["%S".to_string(), ext_word(&path)])
            .collect::<Vec<_>>();
        format!("ext::{}", words.join(" "))
    }
}

/// One word for the `ext::` helper's line: a space is `% `, a percent `%%`.
fn ext_word(w: &str) -> String {
    w.replace('%', "%%").replace(' ', "% ")
}

/// The target for `session`, pure over the parsed config: None when `targets` or the session
/// is absent, Err naming the fault when the entry is there and unusable.
pub fn target_for(cfg: &serde_json::Value, session: &str) -> Result<Option<Target>, String> {
    let Some(entry) = cfg.get("targets").and_then(|t| t.get(session)) else {
        return Ok(None);
    };
    let field = |k: &str| entry.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let kind = field("kind");
    let (exec, shell, label) = match (kind.as_deref(), entry.get("exec")) {
        (Some("ssh"), None) => {
            let host = field("host").ok_or_else(|| format!("targets.{session} has no \"host\""))?;
            let dest = match field("user") {
                Some(u) => format!("{u}@{host}"),
                None => host,
            };
            (vec!["ssh".to_string(), dest.clone()], true, dest)
        }
        (None | Some("exec"), Some(list)) => {
            let words = list
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|w| w.as_str().filter(|s| !s.is_empty()).map(str::to_string))
                        .collect::<Option<Vec<_>>>()
                })
                .unwrap_or_default()
                .filter(|w| !w.is_empty());
            let Some(words) = words else {
                return Err(format!(
                    "targets.{session}.exec must be a list of non-empty words, the argv that \
                     runs a command on the machine, e.g. [\"kubectl\", \"exec\", \"-i\", \
                     \"<pod>\", \"-c\", \"<container>\", \"-n\", \"<ns>\", \"--\"]"
                ));
            };
            let label = words.join(" ");
            (words, false, label)
        }
        (Some("ssh"), Some(_)) => {
            return Err(format!(
                "targets.{session} has both \"kind\": \"ssh\" and an \"exec\" list; keep one"
            ));
        }
        (Some("exec"), None) => {
            return Err(format!(
                "targets.{session}.kind is \"exec\" but there is no \"exec\" list"
            ));
        }
        (Some(other), _) => {
            return Err(format!(
                "targets.{session}.kind is {other:?}; this air knows \"ssh\" (host, user, \
                 path) and an \"exec\" list (the argv prefix that runs a command on the \
                 machine, with path)"
            ));
        }
        (None, None) => {
            return Err(format!(
                "targets.{session} has neither \"kind\": \"ssh\" nor an \"exec\" list"
            ));
        }
    };
    let path = field("path").ok_or_else(|| format!("targets.{session} has no \"path\""))?;
    Ok(Some(Target {
        exec,
        shell,
        label,
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
            row.machine = probe(&mut t.command(PROBE));
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
///
/// The CPU model falls back in order (air-c14): `model name` in `/proc/cpuinfo` (x86 Linux),
/// then `Model name:` from `lscpu`, then `CPU implementer` and `CPU part` from `/proc/cpuinfo`
/// joined by a space, then macOS's `sysctl`. arm64 Linux prints no `model name` line, so the
/// first full pod verify recorded an empty column and the run test was red there
/// (2026-11-05, aarch64, Neoverse-V2 reported by `lscpu` as `0x41 0xd4f` in cpuinfo).
const PROBE: &str = r#"printf '%s
' "$(hostname)" "${AIR_IMAGE:-}" "$(uname -m)" "$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null)" "$(c=$(sed -n 's/^model name[^:]*: *//p' /proc/cpuinfo 2>/dev/null | head -1); [ -n "$c" ] || c=$(lscpu 2>/dev/null | sed -n 's/^Model name:[[:space:]]*//p' | head -1); [ -n "$c" ] || c=$(sed -n 's/^CPU implementer[^:]*: *//p; s/^CPU part[^:]*: *//p' /proc/cpuinfo 2>/dev/null | head -2 | tr '\n' ' '); [ -n "$c" ] || c=$(sysctl -n machdep.cpu.brand_string 2>/dev/null); printf '%s' "$c")""#;

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
    if let Some(code) = prepare_target(repo, session, sha, t) {
        return (code, 0);
    }
    // Through the same pump as `air record`, so the output streams as it arrives and is
    // counted.
    let (prog, args) = t.command_line(&remote_line(t, command));
    match super::record::run_tee(&prog, &args, repo) {
        Ok((code, bytes, _)) => {
            // After the command, whatever its exit: a failed benchmark's partial results are
            // still the results asked for.
            if !pull.is_empty() {
                *pulled = pull::fetch(repo, t, pull);
            }
            (code, bytes)
        }
        Err(e) => {
            eprintln!("air run: {prog}: {e}");
            (127, 0)
        }
    }
}

/// `--pull`: declared result paths back from the target as one tar stream (air-opq.4).
pub mod pull {
    use std::path::Path;
    use std::process::{Command, ExitStatus, Stdio};

    use super::{Target, sh_quote};

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

    /// List, then stream: the target's prefix running `cd <path> && sh -c LIST air <globs>`,
    /// and when anything matched, the prefix running `cd <path> && tar -cf - -- <paths>` piped
    /// into `tar -xf - -C <repo>`. Reports each glob that matched nothing and one line for
    /// what came back; a transport failure is reported and leaves the run's exit code alone.
    pub fn fetch(repo: &Path, t: &Target, globs: &[String]) -> Pulled {
        let path = sh_quote(&t.path);
        let args = globs
            .iter()
            .map(|g| sh_quote(g))
            .collect::<Vec<_>>()
            .join(" ");
        let list = format!("cd {path} && sh -c {} air {args}", sh_quote(LIST));
        let out = t.command(&list).output();
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
                eprintln!("air run: --pull: {}: {e}; nothing pulled", t.dest());
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
        let mut send = match t
            .command(&format!("cd {path} && tar -cf - -- {quoted}"))
            .stdout(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("air run: --pull: {}: {e}; nothing pulled", t.dest());
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
        let sent = send.wait();
        match unpack {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!(
                    "air run: --pull: the tar stream failed (target {}, unpack exit {}); the \
                     worktree may hold part of it",
                    if sent.as_ref().is_ok_and(ExitStatus::success) {
                        "ok"
                    } else {
                        "failed"
                    },
                    s.code().unwrap_or(1)
                );
                return Pulled::default();
            }
            Err(e) => {
                eprintln!("air run: --pull: tar: {e}; nothing pulled");
                return Pulled::default();
            }
        }
        if let Some(code) = sender_failure(&sent) {
            eprintln!(
                "air run: --pull: the target's tar exited {code} after the stream; nothing \
                 recorded as pulled"
            );
            return Pulled::default();
        }
        let bytes = paths.iter().map(|p| size_of(&repo.join(p))).sum();
        eprintln!("air run: pulled {} ({bytes} bytes)", paths.join(", "));
        Pulled { paths, bytes }
    }

    /// Pure: the sender's exit read the way the field reads a tar pipeline's (air-b50,
    /// `docs/research/pull-over-exec.md`): the unpack is the verdict, so a sender killed by
    /// SIGPIPE (signal 13, or exit 141 from a shell) is the normal end, since the local tar
    /// stops at the end-of-archive marker and closes the pipe while the prefix is still
    /// writing the padding. Any other non-zero exit is the target's own `tar` failing, which
    /// an empty stream would otherwise hide because tar exits 0 on one; it is returned as the
    /// code. In the pod trial every pull through `kubectl exec` was reported failed and
    /// recorded empty on the 141 alone. Removal: when the pull is no longer a tar stream.
    pub fn sender_failure(sent: &std::io::Result<ExitStatus>) -> Option<i32> {
        match sent {
            Ok(s) if s.success() || died_of_sigpipe(*s) => None,
            Ok(s) => Some(s.code().unwrap_or(1)),
            Err(_) => Some(1),
        }
    }

    /// Signal 13, or exit code 141 (128 + 13) from a shell that relayed it.
    fn died_of_sigpipe(s: ExitStatus) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if s.signal() == Some(13) {
                return true;
            }
        }
        s.code() == Some(141)
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

        /// air-b50: the sender's exit as the field reads it. Raw wait statuses: a signal is
        /// the low bits, an exit code is shifted by eight.
        #[cfg(unix)]
        #[test]
        fn a_sender_killed_by_sigpipe_is_not_a_failure_and_an_exit_code_is() {
            use std::os::unix::process::ExitStatusExt;
            let status =
                |raw: i32| -> std::io::Result<ExitStatus> { Ok(ExitStatus::from_raw(raw)) };
            assert_eq!(sender_failure(&status(0)), None);
            assert_eq!(sender_failure(&status(13)), None, "signal 13");
            assert_eq!(sender_failure(&status(141 << 8)), None, "exit 141");
            assert_eq!(sender_failure(&status(2 << 8)), Some(2));
            assert_eq!(
                sender_failure(&status(9)),
                Some(1),
                "another signal is a failure"
            );
            assert_eq!(
                sender_failure(&Err(std::io::Error::other("no wait"))),
                Some(1)
            );
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
    let path = sh_quote(&t.path);
    let short = sha.get(..8).unwrap_or(sha);
    eprintln!("air run: {session} at {short} on {}:{}", t.dest(), t.path);
    // Once: a repository at the path. Idempotent, so every run may ask.
    let init = format!("test -d {path}/.git || {{ mkdir -p {path} && git init -q {path}; }}");
    if let Some(code) = step("init", t.command(&init).status()) {
        return Some(code);
    }
    // Force, because the ref is this session's own and an amend or a rebase moves it back.
    // Through the prefix as git's `ext::` helper; `-c` on this one command turns the helper
    // on, and `.git/config` is not written.
    let push = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "protocol.ext.allow=always", "push", "-q", "--force"])
        .arg(t.push_url())
        .arg(format!("{sha}:refs/air/{session}"))
        .stdin(Stdio::null())
        .status();
    if let Some(code) = step("push", push) {
        return Some(code);
    }
    let checkout = format!("cd {path} && git checkout -q --detach {sha}");
    step("checkout", t.command(&checkout).status())
}

/// The shell line that runs `command` in the target's path.
pub fn remote_line(t: &Target, command: &[String]) -> String {
    let cmd = command
        .iter()
        .map(|a| sh_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    format!("cd {} && {cmd}", sh_quote(&t.path))
}

/// The machine a command would run on: the target's, read through its prefix, or this one's.
pub fn probe_machine(t: Option<&Target>) -> Machine {
    match t {
        Some(t) => probe(&mut t.command(PROBE)),
        None => probe(Command::new("sh").arg("-c").arg(PROBE)),
    }
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
        assert_eq!(t.exec, vec!["ssh", "box"], "the short form is a prefix");
        assert!(t.shell);
    }

    #[test]
    fn the_ssh_short_form_quotes_the_line_for_the_remote_shell_and_exec_does_not() {
        let cfg = serde_json::json!({"targets": {
            "w": {"kind": "ssh", "host": "box", "user": "me", "path": "~/air/p"},
            "k": {"exec": ["kubectl", "exec", "-i", "pod", "-c", "me-k8s-ssh", "-n", "ssh", "--"],
                  "path": "/home/me/air/p"},
        }});
        let w = target_for(&cfg, "w").unwrap().unwrap();
        assert_eq!(w.dest(), "me@box");
        assert_eq!(
            w.command_line("cd ~/air/p && make test"),
            (
                "ssh".to_string(),
                vec![
                    "me@box".to_string(),
                    "sh".to_string(),
                    "-c".to_string(),
                    "'cd ~/air/p && make test'".to_string()
                ]
            ),
            "ssh joins its arguments into one remote shell line, so the line is quoted"
        );
        assert_eq!(w.push_url(), "ext::ssh me@box %S ~/air/p");
        let k = target_for(&cfg, "k").unwrap().unwrap();
        assert_eq!(k.dest(), "kubectl exec -i pod -c me-k8s-ssh -n ssh --");
        assert!(!k.shell);
        let (prog, args) = k.command_line("cd /home/me/air/p && make test");
        assert_eq!(prog, "kubectl");
        assert_eq!(
            args,
            vec![
                "exec",
                "-i",
                "pod",
                "-c",
                "me-k8s-ssh",
                "-n",
                "ssh",
                "--",
                "sh",
                "-c",
                "cd /home/me/air/p && make test"
            ],
            "kubectl exec passes argv through, so the line goes verbatim"
        );
        assert_eq!(
            k.push_url(),
            "ext::kubectl exec -i pod -c me-k8s-ssh -n ssh -- %S /home/me/air/p",
            "the push is the prefix with git-receive-pack <path> after it"
        );
    }

    #[test]
    fn push_url_escapes_a_space_and_a_percent_the_way_the_ext_helper_reads_them() {
        let t = Target {
            exec: vec![
                "run".to_string(),
                "pod name".to_string(),
                "100%".to_string(),
            ],
            shell: false,
            label: String::new(),
            path: "/a b".to_string(),
        };
        assert_eq!(t.push_url(), "ext::run pod% name 100%% %S /a% b");
        let s = Target {
            exec: vec!["ssh".to_string(), "box".to_string()],
            shell: true,
            label: String::new(),
            path: "/a b".to_string(),
        };
        assert_eq!(
            s.push_url(),
            "ext::ssh box %S '/a% b'",
            "over ssh the path is quoted for the remote shell as well"
        );
    }

    #[test]
    fn target_faults_are_named() {
        let cfg =
            serde_json::json!({"targets": {"lane": {"kind": "exec", "host": "x", "path": "p"}}});
        let err = target_for(&cfg, "lane").unwrap_err();
        assert!(
            err.contains("targets.lane.kind") && err.contains("\"exec\""),
            "{err}"
        );
        let cfg = serde_json::json!({"targets": {"lane": {"kind": "ssh", "host": "x"}}});
        assert!(target_for(&cfg, "lane").unwrap_err().contains("\"path\""));
        let cfg = serde_json::json!({"targets": {"lane": {"kind": "docker", "path": "p"}}});
        let err = target_for(&cfg, "lane").unwrap_err();
        assert!(
            err.contains("\"docker\"") && err.contains("\"exec\""),
            "{err}"
        );
        let cfg = serde_json::json!({"targets": {"lane": {"exec": [], "path": "p"}}});
        assert!(
            target_for(&cfg, "lane")
                .unwrap_err()
                .contains("non-empty words")
        );
        let cfg = serde_json::json!({"targets": {"lane": {"exec": ["a", ""], "path": "p"}}});
        assert!(
            target_for(&cfg, "lane")
                .unwrap_err()
                .contains("non-empty words")
        );
        let cfg = serde_json::json!({"targets": {"lane": {"exec": "kubectl exec", "path": "p"}}});
        assert!(target_for(&cfg, "lane").unwrap_err().contains("list"));
        let cfg = serde_json::json!({"targets": {"lane": {"path": "p"}}});
        assert!(target_for(&cfg, "lane").unwrap_err().contains("neither"));
        let cfg = serde_json::json!({"targets": {"lane": {"kind": "ssh", "host": "x", "exec": ["y"], "path": "p"}}});
        assert!(target_for(&cfg, "lane").unwrap_err().contains("both"));
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
