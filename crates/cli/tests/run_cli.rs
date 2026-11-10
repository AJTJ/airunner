//! `air run` against a stand-in `ssh` on PATH that runs the remote command locally, so the
//! push, the checkout and the run all land in a temp directory and need no sshd (air-opq.1),
//! and against a stand-in `kubectl exec` named by an `exec` prefix (air-tfi).
//! The test names nest under `run::skeleton` so `cargo test -p air -- run::skeleton` selects
//! them.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

mod run {
    pub mod skeleton {
        use std::path::{Path, PathBuf};

        use crate::common;

        pub fn git(dir: &Path, args: &[&str]) -> String {
            let out = common::command("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }

        pub fn scratch_repo() -> tempfile::TempDir {
            let dir = tempfile::tempdir().unwrap();
            git(dir.path(), &["init", "-q", "-b", "main"]);
            std::fs::write(dir.path().join("note"), "first\n").unwrap();
            git(dir.path(), &["add", "note"]);
            git(dir.path(), &["commit", "-q", "-m", "a"]);
            dir
        }

        /// A stand-in `ssh`: appends its argv to `$AIR_TEST_SSH_LOG`, drops the options and
        /// the host, and runs the command locally. git's own push goes through it too, as
        /// `ssh [-o …] host "git-receive-pack '<path>'"`.
        pub fn fake_ssh() -> PathBuf {
            static DIR: std::sync::OnceLock<(tempfile::TempDir, PathBuf)> =
                std::sync::OnceLock::new();
            DIR.get_or_init(|| {
                let home = tempfile::tempdir().unwrap();
                let script = home.path().join("ssh");
                std::fs::write(
                    &script,
                    r#"#!/bin/sh
printf '%s\n' "$*" >> "$AIR_TEST_SSH_LOG"
while [ $# -gt 0 ]; do
  case "$1" in
    -o|-p|-l|-i|-F) shift 2 ;;
    -*) shift ;;
    *) break ;;
  esac
done
shift
exec sh -c "$*"
"#,
                )
                .unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                }
                let dir = home.path().to_path_buf();
                (home, dir)
            })
            .1
            .clone()
        }

        /// `air --repo <repo> run -- <command>` with the stand-in first on PATH and git's own
        /// ssh overrides cleared, so `ssh` is what PATH says.
        pub fn air_run(repo: &Path, command: &[&str]) -> (i32, String, String) {
            air_run_with(repo, command, &[], false)
        }

        /// [`air_run`] with extra environment and the global `--json`.
        pub fn air_run_with(
            repo: &Path,
            command: &[&str],
            env: &[(&str, &str)],
            json: bool,
        ) -> (i32, String, String) {
            air_run_args(repo, &[], command, env, json)
        }

        /// [`air_run_with`] with `run`'s own arguments (`--pull …`) before the `--`.
        pub fn air_run_args(
            repo: &Path,
            run_args: &[&str],
            command: &[&str],
            env: &[(&str, &str)],
            json: bool,
        ) -> (i32, String, String) {
            let path = format!(
                "{}:{}",
                fake_ssh().display(),
                std::env::var("PATH").unwrap_or_default()
            );
            let mut cmd = common::command(env!("CARGO_BIN_EXE_air"));
            cmd.arg("--repo").arg(repo);
            if json {
                cmd.arg("--json");
            }
            for (k, v) in env {
                cmd.env(k, v);
            }
            let out = cmd
                .arg("run")
                .args(run_args)
                .arg("--")
                .args(command)
                .env("PATH", path)
                .env("AIR_TEST_SSH_LOG", repo.join("ssh.log"))
                .env("AIR_TEST_EXEC_LOG", repo.join("exec.log"))
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env_remove("GIT_SSH")
                .env_remove("GIT_SSH_COMMAND")
                .current_dir(repo)
                .output()
                .unwrap();
            (
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout).to_string(),
                String::from_utf8_lossy(&out.stderr).to_string(),
            )
        }

        pub fn ssh_log(repo: &Path) -> String {
            std::fs::read_to_string(repo.join("ssh.log")).unwrap_or_default()
        }

        /// A stand-in for `kubectl exec -i <pod> -c <c> -n ssh --`: appends its argv to
        /// `$AIR_TEST_EXEC_LOG`, one word per line and `<end>` after each call, drops its own
        /// words up to `--`, and runs the rest here as argv with no shell between. `--fail <n>`
        /// exits `n` before running anything, the way a gone pod or an expired token would.
        /// `--tar <mode>` shapes the `--pull` stream's sender (air-b50): `sigpipe141` and
        /// `sigpipe13` send the whole archive and then exit 141 or die of SIGPIPE, the way
        /// `kubectl exec` ends once the local tar has closed the pipe; `empty2` sends nothing
        /// and exits 2, a target-side tar that could not read a file; `truncate` cuts the
        /// stream inside a member.
        pub fn fake_exec() -> PathBuf {
            static DIR: std::sync::OnceLock<(tempfile::TempDir, PathBuf)> =
                std::sync::OnceLock::new();
            DIR.get_or_init(|| {
                let home = tempfile::tempdir().unwrap();
                let script = home.path().join("fake-kubectl");
                std::fs::write(
                    &script,
                    r#"#!/bin/sh
for a in "$@"; do printf '%s\n' "$a"; done >> "$AIR_TEST_EXEC_LOG"
printf '<end>\n' >> "$AIR_TEST_EXEC_LOG"
tarmode=
while [ $# -gt 0 ]; do
  case "$1" in
    --fail) exit "$2" ;;
    --tar) tarmode=$2; shift 2 ;;
    --) shift; break ;;
    *) shift ;;
  esac
done
case "$tarmode:$*" in
  sigpipe141:*"tar -cf -"*) "$@"; exit 141 ;;
  sigpipe13:*"tar -cf -"*) "$@"; kill -13 $$ ;;
  empty2:*"tar -cf -"*) exit 2 ;;
  truncate:*"tar -cf -"*) "$@" | head -c 700; exit 0 ;;
esac
exec "$@"
"#,
                )
                .unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                }
                (home, script)
            })
            .1
            .clone()
        }

        pub fn exec_log(repo: &Path) -> String {
            std::fs::read_to_string(repo.join("exec.log")).unwrap_or_default()
        }

        /// `.claude/air.json` with an `exec` target for `main` at `<repo>/target`: the stand-in
        /// with `words` as its own arguments before `--`. One word carries a space and a
        /// percent, the two characters the `ext::` helper treats specially, so the push proves
        /// they arrive as one word.
        pub fn exec_target(repo: &Path, words: &[&str]) -> (PathBuf, Vec<String>) {
            let target = repo.join("target");
            let mut exec = vec![fake_exec().display().to_string()];
            exec.extend(words.iter().map(|w| w.to_string()));
            exec.push("--".to_string());
            std::fs::create_dir_all(repo.join(".claude")).unwrap();
            std::fs::write(
                repo.join(".claude/air.json"),
                serde_json::json!({"targets": {"main": {
                    "exec": exec, "path": target.display().to_string(),
                }}})
                .to_string(),
            )
            .unwrap();
            (target, exec)
        }

        /// The remotes `.git/config` names, which a push through the transport leaves alone.
        pub fn remotes(repo: &Path) -> String {
            let out = common::command("git")
                .arg("-C")
                .arg(repo)
                .args(["config", "--get-regexp", r"^remote\."])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).to_string()
        }

        /// Any `air` subcommand from `repo`, with the stand-in ssh on PATH as [`air_run_args`].
        pub fn air_cmd(repo: &Path, args: &[&str]) -> (i32, String, String) {
            let path = format!(
                "{}:{}",
                fake_ssh().display(),
                std::env::var("PATH").unwrap_or_default()
            );
            let out = common::command(env!("CARGO_BIN_EXE_air"))
                .arg("--repo")
                .arg(repo)
                .args(args)
                .env("PATH", path)
                .env("AIR_TEST_SSH_LOG", repo.join("ssh.log"))
                .env("AIR_TEST_EXEC_LOG", repo.join("exec.log"))
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env_remove("GIT_SSH")
                .env_remove("GIT_SSH_COMMAND")
                .current_dir(repo)
                .output()
                .unwrap();
            (
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stdout).to_string(),
                String::from_utf8_lossy(&out.stderr).to_string(),
            )
        }

        /// `.claude/air.json` with an ssh target for `main` at `<repo>/target`.
        pub fn ssh_target(repo: &Path) -> PathBuf {
            let target = repo.join("target");
            std::fs::create_dir_all(repo.join(".claude")).unwrap();
            std::fs::write(
                repo.join(".claude/air.json"),
                serde_json::json!({"targets": {"main": {
                    "kind": "ssh", "host": "box", "user": "me",
                    "path": target.display().to_string(),
                }}})
                .to_string(),
            )
            .unwrap();
            target
        }

        #[test]
        fn no_target_runs_locally_with_the_commands_exit_code() {
            let repo = scratch_repo();
            let (code, out, err) = air_run(
                repo.path(),
                &["sh", "-c", "cat note; echo to-err >&2; exit 3"],
            );
            assert_eq!(code, 3);
            assert_eq!(out, "first\n");
            assert_eq!(err, "to-err\n", "nothing added to the command's own output");
            assert_eq!(ssh_log(repo.path()), "", "no ssh without a target");
        }

        #[test]
        fn no_target_names_a_command_it_cannot_start() {
            let repo = scratch_repo();
            let (code, _, err) = air_run(repo.path(), &["no-such-program-air"]);
            assert_eq!(code, 127);
            assert!(err.contains("no-such-program-air"), "{err}");
        }

        #[test]
        fn ssh_target_initialises_once_pushes_checks_out_and_runs_there() {
            let repo = scratch_repo();
            let target = ssh_target(repo.path());
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);

            let (code, out, err) = air_run(
                repo.path(),
                &[
                    "sh",
                    "-c",
                    "git rev-parse HEAD; cat note; echo to-err >&2; exit 4",
                ],
            );
            assert_eq!(code, 4, "the command's exit comes back\n{err}");
            assert_eq!(
                out,
                format!("{sha}\nfirst\n"),
                "ran at HEAD's sha in the target"
            );
            assert!(
                err.contains("to-err"),
                "the command's stderr comes back: {err}"
            );
            assert!(
                err.contains("me@box"),
                "one line names the target before the run: {err}"
            );
            assert!(target.join(".git").is_dir(), "the target was initialised");
            assert_eq!(
                git(&target, &["rev-parse", "refs/air/main"]),
                sha,
                "pushed to the session's own ref"
            );
            let log = ssh_log(repo.path());
            assert!(log.contains("git init -q"), "{log}");
            assert!(
                log.contains("git-receive-pack"),
                "the push went over ssh: {log}"
            );
            assert!(
                log.contains(&format!(
                    "git clean -qfd && git checkout -q -f --detach {sha}"
                )),
                "{log}"
            );
            assert!(
                !log.contains("git push"),
                "ssh carries receive-pack, not a push command: {log}"
            );
            assert_eq!(
                remotes(repo.path()),
                "",
                "the push names no remote and adds none to .git/config"
            );

            // A second run: an amended commit, so the ref moves backwards and the push must be
            // forced; the repository on the target is not made again.
            std::fs::write(target.join(".git/air-marker"), "kept\n").unwrap();
            std::fs::write(repo.path().join("note"), "second\n").unwrap();
            git(repo.path(), &["commit", "-q", "-a", "--amend", "-m", "b"]);
            let sha2 = git(repo.path(), &["rev-parse", "HEAD"]);
            assert_ne!(sha, sha2);
            let (code, out, _) = air_run(repo.path(), &["cat", "note"]);
            assert_eq!(code, 0);
            assert_eq!(out, "second\n", "the amended commit is what ran");
            assert_eq!(git(&target, &["rev-parse", "refs/air/main"]), sha2);
            assert_eq!(git(&target, &["rev-parse", "HEAD"]), sha2);
            assert!(
                target.join(".git/air-marker").exists(),
                "initialised once: the target repository survived the second run"
            );
        }

        #[test]
        fn a_target_of_another_kind_is_refused_by_name() {
            let repo = scratch_repo();
            std::fs::create_dir_all(repo.path().join(".claude")).unwrap();
            std::fs::write(
                repo.path().join(".claude/air.json"),
                r#"{"targets": {"main": {"kind": "exec", "host": "pod", "path": "p"}}}"#,
            )
            .unwrap();
            let (code, _, err) = air_run(repo.path(), &["true"]);
            assert_eq!(code, 2);
            assert!(err.contains("targets.main.kind"), "{err}");
            assert_eq!(ssh_log(repo.path()), "", "nothing ran");
        }
    }

    /// A target is an argv prefix, and ssh is one spelling of it (air-tfi): the stand-in for
    /// `kubectl exec` passes argv through with no shell, and every step, the push included,
    /// goes through it.
    pub mod exec {
        use air_ledger::verify::Kind;

        use super::record::split;
        use super::skeleton::{
            air_cmd, air_run_args, exec_log, exec_target, git, remotes, scratch_repo, ssh_log,
        };

        const WORDS: &[&str] = &["-i", "pod", "-c", "me-k8s-ssh", "-n", "100% ssh"];

        #[test]
        fn an_exec_prefix_pushes_runs_and_pulls_through_it_and_the_row_names_it() {
            let repo = scratch_repo();
            let (target, exec) = exec_target(repo.path(), WORDS);
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);
            let (code, out, err) = air_run_args(
                repo.path(),
                &["--pull", "results/*"],
                &[
                    "sh",
                    "-c",
                    "git rev-parse HEAD; mkdir -p results; printf hi > results/a; exit 4",
                ],
                &[],
                true,
            );
            assert_eq!(code, 4, "the command's exit comes back\n{err}");
            let (printed, row) = split(&out);
            assert_eq!(
                printed,
                format!("{sha}\n"),
                "ran at HEAD's sha in the target"
            );
            assert_eq!(
                row.target,
                format!("{}:{}", exec.join(" "), target.display()),
                "the row names the prefix"
            );
            assert_eq!(row.exit_code, 4);
            assert_eq!(row.pulled, vec!["results/a".to_string()]);
            assert_eq!(
                std::fs::read_to_string(repo.path().join("results/a")).unwrap(),
                "hi",
                "pulled back through the prefix"
            );
            assert!(
                !row.machine.hostname.is_empty(),
                "the probe ran there: {row:?}"
            );
            assert!(
                err.contains(&format!("on {}:", exec.join(" "))),
                "one line names the target before the run: {err}"
            );
            assert_eq!(git(&target, &["rev-parse", "refs/air/main"]), sha);
            assert_eq!(git(&target, &["rev-parse", "HEAD"]), sha);
            assert_eq!(ssh_log(repo.path()), "", "no ssh anywhere");
            assert_eq!(remotes(repo.path()), "", "no remote named or added");

            // Each call is the prefix's words, then what Air appended: `sh -c <line>` with
            // the line verbatim, or git's `git-receive-pack <path>` for the push.
            let log = exec_log(repo.path());
            let calls: Vec<Vec<&str>> = log
                .split("<end>\n")
                .filter(|c| !c.is_empty())
                .map(|c| c.lines().collect())
                .collect();
            let prefix = ["-i", "pod", "-c", "me-k8s-ssh", "-n", "100% ssh", "--"];
            for call in &calls {
                assert_eq!(
                    &call[..7],
                    &prefix,
                    "every call starts with the prefix, a space and a percent intact: {call:?}"
                );
            }
            let steps: Vec<&str> = calls.iter().map(|c| c[7]).collect();
            assert_eq!(
                steps,
                vec!["sh", "sh", "git-receive-pack", "sh", "sh", "sh", "sh"],
                "probe, init, push, checkout, run, list, tar: {log}"
            );
            let push = &calls[2];
            assert_eq!(push[8], target.display().to_string(), "{push:?}");
            let run = &calls[4];
            assert_eq!(run[8], "-c");
            assert!(
                run[9].starts_with(&format!("cd {} && sh -c ", target.display())),
                "the line arrives as one unquoted word: {run:?}"
            );
            assert!(
                calls[6][9].contains("tar -cf - -- results/a"),
                "{:?}",
                calls[6]
            );
        }

        #[test]
        fn a_verify_through_an_exec_prefix_is_green_at_the_local_head() {
            let repo = scratch_repo();
            let (target, exec) = exec_target(repo.path(), WORDS);
            git(repo.path(), &["add", ".claude/air.json"]);
            git(repo.path(), &["commit", "-q", "-m", "config"]);
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);
            let (code, out, err) = air_cmd(
                repo.path(),
                &["record", "verify", "--", "sh", "-c", "git rev-parse HEAD"],
            );
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains(&sha), "ran at the pushed commit: {out}");
            assert!(out.contains("recorded green verify"), "{out}");
            assert!(
                err.contains(&format!("on {} (", exec.join(" "))),
                "the flag names the prefix and the hostname: {err}"
            );
            assert_eq!(git(&target, &["rev-parse", "HEAD"]), sha);
            let ledger = air_ledger::Ledger::open_in(&repo.path().join(".air")).unwrap();
            assert_eq!(ledger.runs_at(&sha, Kind::Verify).unwrap(), (1, 0));
            let run = ledger
                .latest_run_any("main", Kind::Verify)
                .unwrap()
                .unwrap();
            assert_eq!(run.sha, sha, "recorded at the local head");
            assert!(!run.dirty);
            let m = ledger.machine_of(&run.id).unwrap().unwrap();
            assert!(!m.hostname.is_empty() && m.nproc > 0, "{m:?}");
            assert_eq!(ssh_log(repo.path()), "", "no ssh anywhere");
        }

        #[test]
        fn a_transport_that_fails_before_the_command_names_the_step_and_records_no_green() {
            let repo = scratch_repo();
            let (target, _) = exec_target(repo.path(), &["--fail", "9"]);
            git(repo.path(), &["add", ".claude/air.json"]);
            git(repo.path(), &["commit", "-q", "-m", "config"]);
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);

            let (code, out, err) = air_cmd(repo.path(), &["record", "verify", "--", "true"]);
            assert_eq!(
                code, 9,
                "the transport's exit, not the command's\n{out}{err}"
            );
            assert!(
                err.contains("init failed (exit 9)"),
                "the step is named: {err}"
            );
            assert!(
                err.contains("NOTHING was recorded"),
                "a failed transport is no verdict: {err}"
            );
            assert!(!target.exists(), "nothing was made on the target");
            let ledger = air_ledger::Ledger::open_in(&repo.path().join(".air")).unwrap();
            assert_eq!(
                ledger.runs_at(&sha, Kind::Verify).unwrap(),
                (0, 0),
                "no green and no red"
            );
            assert_eq!(remotes(repo.path()), "");

            // The stand-in's log is untracked now, so `air run`, which takes a dirty tree,
            // goes second.
            let (code, out, err) = air_run_args(repo.path(), &[], &["true"], &[], true);
            assert_eq!(code, 9, "{err}");
            assert!(err.contains("init failed (exit 9)"), "{err}");
            let (_, row) = split(&out);
            assert_eq!(row.exit_code, 9, "the run row records the failure");
            assert_eq!(
                row.machine.hostname, "",
                "the probe failed too, and says so"
            );
        }
    }

    /// Every run is one row in `runs`, with the machine the probe read there (air-opq.3).
    pub mod record {
        use super::skeleton::{air_run_with, git, scratch_repo, ssh_target};

        /// The command's own output comes first, then the row.
        pub fn split(stdout: &str) -> (String, air_ledger::runs::Run) {
            let at = stdout.find('{').unwrap();
            let (out, json) = stdout.split_at(at);
            (out.to_string(), serde_json::from_str(json).unwrap())
        }

        #[test]
        fn a_run_through_the_target_records_commit_command_exit_and_machine() {
            let repo = scratch_repo();
            let target = ssh_target(repo.path());
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);
            let tree = git(repo.path(), &["rev-parse", "HEAD^{tree}"]);
            let (code, out, _) = air_run_with(
                repo.path(),
                &["sh", "-c", "echo hi; exit 3"],
                &[("AIR_IMAGE", "dev:1")],
                true,
            );
            assert_eq!(code, 3);
            let (printed, row) = split(&out);
            assert_eq!(printed, "hi\n");
            assert_eq!(row.session, "main");
            assert_eq!(row.sha, sha);
            assert_eq!(row.tree.as_deref(), Some(tree.as_str()));
            assert_eq!(row.command, "sh -c echo hi; exit 3");
            assert_eq!(row.exit_code, 3);
            assert_eq!(row.output_bytes, 3);
            assert_eq!(row.target, format!("me@box:{}", target.display()));
            assert!(!row.started_at.is_empty() && !row.finished_at.is_empty());
            let m = &row.machine;
            assert!(!m.hostname.is_empty(), "hostname: {m:?}");
            assert_eq!(
                m.image.as_deref(),
                Some("dev:1"),
                "the stand-in reports this machine"
            );
            assert!(!m.arch.is_empty(), "arch: {m:?}");
            assert!(m.nproc > 0, "nproc: {m:?}");
            assert!(!m.cpu.is_empty(), "cpu: {m:?}");
            let ledger = air_ledger::Ledger::open_in(&repo.path().join(".air")).unwrap();
            assert_eq!(
                ledger.runs_of("main").unwrap(),
                vec![row],
                "the printed row is the stored one"
            );
        }

        #[test]
        fn a_local_run_records_this_machine_as_local() {
            let repo = scratch_repo();
            let (code, out, err) = air_run_with(repo.path(), &["sh", "-c", "exit 5"], &[], true);
            assert_eq!(code, 5);
            assert_eq!(err, "", "nothing added");
            let (printed, row) = split(&out);
            assert_eq!(printed, "");
            assert_eq!(row.target, "local");
            assert_eq!(row.exit_code, 5);
            assert_eq!(row.output_bytes, 0);
            assert!(!row.machine.hostname.is_empty());
            assert!(row.machine.nproc > 0);
            assert_eq!(
                row.machine.image, None,
                "no image unless the machine names one"
            );
            let ledger = air_ledger::Ledger::open_in(&repo.path().join(".air")).unwrap();
            assert_eq!(ledger.runs_of("main").unwrap().len(), 1);
        }
    }

    /// `--pull`: declared result paths come back as one tar stream and are recorded
    /// (air-opq.4).
    pub mod pull {
        use super::record::split;
        use super::skeleton::{air_run_args, exec_target, git, scratch_repo, ssh_log, ssh_target};

        const WRITE: &str = "mkdir -p results other; printf hello > results/a.txt; \
                             printf nope > results/b.log; printf x > other/c.txt; exit 3";

        #[test]
        fn declared_paths_come_back_overwriting_and_the_record_names_them() {
            let repo = scratch_repo();
            let target = ssh_target(repo.path());
            std::fs::create_dir_all(repo.path().join("results")).unwrap();
            std::fs::write(repo.path().join("results/a.txt"), "old\n").unwrap();

            let (code, out, err) = air_run_args(
                repo.path(),
                &["--pull", "results/*.txt", "--pull", "nothing/*"],
                &["sh", "-c", WRITE],
                &[],
                true,
            );
            assert_eq!(code, 3, "the command's exit, not the pull's\n{err}");
            assert_eq!(
                std::fs::read_to_string(repo.path().join("results/a.txt")).unwrap(),
                "hello",
                "pulled back with the same content, overwriting"
            );
            assert!(
                !repo.path().join("results/b.log").exists(),
                "a path the glob does not name stays on the target"
            );
            assert!(
                !repo.path().join("other").exists(),
                "nothing undeclared comes back"
            );
            assert_eq!(
                std::fs::read_to_string(target.join("results/b.log")).unwrap(),
                "nope",
                "the target keeps what it wrote"
            );
            assert!(
                err.contains("--pull nothing/*: matched nothing"),
                "a glob with no match is reported: {err}"
            );
            assert!(
                err.contains("pulled results/a.txt (5 bytes)"),
                "one line says what came back: {err}"
            );
            let (_, row) = split(&out);
            assert_eq!(row.pulled, vec!["results/a.txt".to_string()]);
            assert_eq!(row.pulled_bytes, 5);
            assert_eq!(row.exit_code, 3);
            let ledger = air_ledger::Ledger::open_in(&repo.path().join(".air")).unwrap();
            let stored = ledger.runs_of("main").unwrap();
            assert_eq!(stored.len(), 1);
            assert_eq!(
                stored[0].pulled, row.pulled,
                "the stored row names the path"
            );
            assert_eq!(stored[0].pulled_bytes, 5);
            let log = ssh_log(repo.path());
            assert!(
                log.contains("tar -cf - -- results/a.txt"),
                "one tar stream over the same transport: {log}"
            );
        }

        /// air-b50 (pod trial, 2026-11-06): every pull through `kubectl exec` was reported as
        /// "the tar stream failed" and recorded as empty, though the file had arrived. The local
        /// tar stops at the end-of-archive marker and closes the pipe, and the prefix dies of
        /// SIGPIPE writing the padding (exit 141 from a shell, signal 13 from a program). The
        /// verdict is the unpack's, as `kubectl cp`'s is; a sender that exits with a code is
        /// still a failure, since tar exits 0 on an empty stream (docs/research/pull-over-exec.md).
        #[test]
        fn the_unpack_is_the_verdict_and_a_sender_dying_of_sigpipe_is_the_normal_end() {
            let write = "mkdir -p results; printf hi > results/a";
            for mode in ["sigpipe141", "sigpipe13"] {
                let repo = scratch_repo();
                exec_target(repo.path(), &["--tar", mode]);
                let (code, out, err) = air_run_args(
                    repo.path(),
                    &["--pull", "results/*"],
                    &["sh", "-c", write],
                    &[],
                    true,
                );
                assert_eq!(code, 0, "{mode}: {err}");
                let (_, row) = split(&out);
                assert_eq!(row.pulled, vec!["results/a".to_string()], "{mode}: {err}");
                assert_eq!(row.pulled_bytes, 2, "{mode}");
                assert_eq!(
                    std::fs::read_to_string(repo.path().join("results/a")).unwrap(),
                    "hi",
                    "{mode}"
                );
                assert!(
                    !err.contains("tar stream failed")
                        && err.contains("pulled results/a (2 bytes)"),
                    "{mode}: a complete archive is a good pull: {err}"
                );
            }

            // A sender that exits with a code sent nothing it could not read. bsdtar (macOS)
            // exits 0 on an empty stream, so the code is the only sign and it is reported; GNU
            // tar (Linux, the pod) refuses the empty stream itself. Either way nothing is
            // recorded as pulled.
            let repo = scratch_repo();
            exec_target(repo.path(), &["--tar", "empty2"]);
            let (code, out, err) = air_run_args(
                repo.path(),
                &["--pull", "results/*"],
                &["sh", "-c", write],
                &[],
                true,
            );
            assert_eq!(code, 0, "the command's exit, not the pull's: {err}");
            let (_, row) = split(&out);
            assert!(row.pulled.is_empty(), "{err}");
            assert_eq!(row.pulled_bytes, 0);
            assert!(
                (err.contains("exited 2") || err.contains("the tar stream failed"))
                    && !err.contains("pulled results"),
                "the failure is reported, by the sender's code or the unpack: {err}"
            );

            // A stream cut inside a member fails on the unpack, whatever the sender said.
            let repo = scratch_repo();
            exec_target(repo.path(), &["--tar", "truncate"]);
            let (_, out, err) = air_run_args(
                repo.path(),
                &["--pull", "results/*"],
                &["sh", "-c", write],
                &[],
                true,
            );
            let (_, row) = split(&out);
            assert!(row.pulled.is_empty(), "{err}");
            assert!(
                err.contains("the tar stream failed") && !err.contains("pulled results"),
                "{err}"
            );
        }

        #[test]
        fn a_directory_glob_pulls_the_directory_and_sums_its_bytes() {
            let repo = scratch_repo();
            ssh_target(repo.path());
            let (code, out, _) = air_run_args(
                repo.path(),
                &["--pull", "results"],
                &[
                    "sh",
                    "-c",
                    "mkdir -p results/sub; printf ab > results/x; printf cde > results/sub/y",
                ],
                &[],
                true,
            );
            assert_eq!(code, 0);
            assert_eq!(
                std::fs::read_to_string(repo.path().join("results/sub/y")).unwrap(),
                "cde"
            );
            let (_, row) = split(&out);
            assert_eq!(row.pulled, vec!["results".to_string()]);
            assert_eq!(row.pulled_bytes, 5);
        }

        #[test]
        fn nothing_comes_back_without_pull() {
            let repo = scratch_repo();
            ssh_target(repo.path());
            let (code, out, err) = air_run_args(repo.path(), &[], &["sh", "-c", WRITE], &[], true);
            assert_eq!(code, 3);
            assert!(!repo.path().join("results").exists(), "{err}");
            assert!(!err.contains("pulled"), "{err}");
            let (_, row) = split(&out);
            assert!(row.pulled.is_empty());
            assert_eq!(row.pulled_bytes, 0);
            assert!(
                !ssh_log(repo.path()).contains("tar "),
                "no stream without --pull"
            );
        }

        /// air-32u: a run's untracked output stayed in the target checkout, and git refused
        /// the next checkout of a commit that tracks it, so the precheck or the lane's verify
        /// recorded nothing (the pod trial, 2026-11-06). The target holds only commits: what
        /// the last command wrote goes before the checkout, a tracked file it modified is
        /// overwritten, and ignored files stay.
        #[test]
        fn a_runs_output_on_the_target_does_not_block_the_next_checkout_and_ignored_files_stay() {
            let repo = scratch_repo();
            std::fs::write(repo.path().join(".gitignore"), "cache/\n").unwrap();
            git(repo.path(), &["add", ".gitignore"]);
            git(repo.path(), &["commit", "-q", "-m", "ignore cache"]);
            let (target, _) = exec_target(repo.path(), &["-i", "pod"]);
            // Run one writes a result, scribbles on a tracked file and leaves an ignored file.
            let (code, _, err) = air_run_args(
                repo.path(),
                &["--pull", "out.txt"],
                &[
                    "sh",
                    "-c",
                    "echo hi > out.txt; echo scribble >> note; mkdir -p cache; echo c > cache/x",
                ],
                &[],
                true,
            );
            assert_eq!(code, 0, "{err}");
            assert_eq!(
                std::fs::read_to_string(repo.path().join("out.txt")).unwrap(),
                "hi\n"
            );
            // The session commits the pulled result, as the trial's worker did.
            git(repo.path(), &["add", "out.txt"]);
            git(repo.path(), &["commit", "-q", "-m", "result"]);
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);
            // Before the fix: "The following untracked working tree files would be
            // overwritten by checkout" and `air run: checkout failed (exit 1)`.
            let (code, out, err) = air_run_args(
                repo.path(),
                &[],
                &[
                    "sh",
                    "-c",
                    "git rev-parse HEAD; cat out.txt; git status --porcelain; \
                     test -f cache/x && echo kept",
                ],
                &[],
                true,
            );
            assert_eq!(code, 0, "{err}");
            assert!(!err.contains("checkout failed"), "{err}");
            let (printed, row) = split(&out);
            assert_eq!(
                printed,
                format!("{sha}\nhi\nkept\n"),
                "the commit is checked out, the tree is clean, the ignored file stayed"
            );
            assert_eq!(row.exit_code, 0);
            assert_eq!(git(&target, &["rev-parse", "HEAD"]), sha);
        }

        #[test]
        fn a_local_run_pulls_nothing_because_the_files_are_here() {
            let repo = scratch_repo();
            let (code, out, err) = air_run_args(
                repo.path(),
                &["--pull", "results/*"],
                &["sh", "-c", "mkdir -p results; printf hi > results/a"],
                &[],
                true,
            );
            assert_eq!(code, 0);
            assert_eq!(err, "", "nothing added to a local run");
            assert_eq!(
                std::fs::read_to_string(repo.path().join("results/a")).unwrap(),
                "hi"
            );
            let (_, row) = split(&out);
            assert!(row.pulled.is_empty());
        }
    }

    /// `air record` through the runner (air-opq.5): the check runs on the target, the green is
    /// recorded at the local head with the machine beside it, and the close gate reads it as
    /// any other green. A dirty tree is refused; `--local` is the old path.
    pub mod verify {
        use air_ledger::verify::Kind;

        use super::skeleton::{air_cmd, git, scratch_repo, ssh_log, ssh_target};

        #[test]
        fn a_verify_with_a_target_runs_there_and_is_green_at_the_local_head() {
            let repo = scratch_repo();
            let target = ssh_target(repo.path());
            // Tracked, as an adopter's is: an untracked config would itself be dirt.
            git(repo.path(), &["add", ".claude/air.json"]);
            git(repo.path(), &["commit", "-q", "-m", "config"]);
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);
            let (code, out, err) = air_cmd(
                repo.path(),
                &[
                    "record",
                    "verify",
                    "--",
                    "sh",
                    "-c",
                    "git rev-parse HEAD; test -f note",
                ],
            );
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains(&sha), "ran at the pushed commit: {out}");
            assert!(out.contains("recorded green verify"), "{out}");
            assert!(
                err.contains("on me@box"),
                "the flag names the target: {err}"
            );
            let log = ssh_log(repo.path());
            assert!(
                log.contains(&format!(
                    "git clean -qfd && git checkout -q -f --detach {sha}"
                )),
                "{log}"
            );
            assert!(
                log.contains(&format!("cd {} && sh -c", target.display())),
                "the check ran in the target's path: {log}"
            );
            let ledger = air_ledger::Ledger::open_in(&repo.path().join(".air")).unwrap();
            assert_eq!(ledger.runs_at(&sha, Kind::Verify).unwrap(), (1, 0));
            let run = ledger
                .latest_run_any("main", Kind::Verify)
                .unwrap()
                .unwrap();
            assert_eq!(run.sha, sha, "recorded at the local head");
            assert_eq!(
                run.tree.as_deref(),
                Some(git(repo.path(), &["rev-parse", "HEAD^{tree}"]).as_str())
            );
            assert_eq!(
                run.command.as_deref(),
                Some("sh -c git rev-parse HEAD; test -f note")
            );
            assert!(!run.dirty);
            let m = ledger.machine_of(&run.id).unwrap().unwrap();
            assert!(
                !m.hostname.is_empty() && !m.arch.is_empty() && m.nproc > 0,
                "{m:?}"
            );
            // The gate reads it: handover no longer wants a green at HEAD.
            let (_, out, err) = air_cmd(repo.path(), &["handover"]);
            assert!(
                !format!("{out}{err}").contains("no green verify recorded at HEAD"),
                "{out}{err}"
            );
        }

        #[test]
        fn a_dirty_tree_with_a_target_is_refused_and_local_runs_here() {
            let repo = scratch_repo();
            ssh_target(repo.path());
            git(repo.path(), &["add", ".claude/air.json"]);
            git(repo.path(), &["commit", "-q", "-m", "config"]);
            let sha = git(repo.path(), &["rev-parse", "HEAD"]);
            std::fs::write(repo.path().join("note"), "edited\n").unwrap();
            let (code, out, err) = air_cmd(repo.path(), &["record", "verify", "--", "true"]);
            assert_eq!(code, 2, "{out}{err}");
            assert!(err.contains("note") && err.contains("--local"), "{err}");
            assert_eq!(ssh_log(repo.path()), "", "nothing was pushed");
            let ledger = air_ledger::Ledger::open_in(&repo.path().join(".air")).unwrap();
            assert_eq!(
                ledger.runs_at(&sha, Kind::Verify).unwrap(),
                (0, 0),
                "nothing recorded"
            );

            let (code, out, err) =
                air_cmd(repo.path(), &["record", "verify", "--local", "--", "true"]);
            assert_eq!(code, 0, "{out}{err}");
            assert_eq!(ssh_log(repo.path()), "", "--local touches no target");
            assert!(!err.contains("on me@box"), "{err}");
            assert_eq!(ledger.runs_at(&sha, Kind::Verify).unwrap(), (1, 0));
            let run = ledger
                .latest_run_any("main", Kind::Verify)
                .unwrap()
                .unwrap();
            assert!(run.dirty, "the local path records a dirty tree as before");
            let m = ledger.machine_of(&run.id).unwrap().unwrap();
            assert!(!m.hostname.is_empty(), "{m:?}");
        }
    }
}
