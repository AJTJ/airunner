//! `air run` against a stand-in `ssh` on PATH that runs the remote command locally, so the
//! push, the checkout and the run all land in a temp directory and need no sshd (air-opq.1).
//! The test names nest under `run::skeleton` so `cargo test -p air -- run::skeleton` selects
//! them.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

mod run {
    pub mod skeleton {
        use std::path::{Path, PathBuf};

        use crate::common;

        fn git(dir: &Path, args: &[&str]) -> String {
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

        fn scratch_repo() -> tempfile::TempDir {
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
        fn fake_ssh() -> PathBuf {
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
        fn air_run(repo: &Path, command: &[&str]) -> (i32, String, String) {
            let path = format!(
                "{}:{}",
                fake_ssh().display(),
                std::env::var("PATH").unwrap_or_default()
            );
            let out = common::command(env!("CARGO_BIN_EXE_air"))
                .arg("--repo")
                .arg(repo)
                .arg("run")
                .arg("--")
                .args(command)
                .env("PATH", path)
                .env("AIR_TEST_SSH_LOG", repo.join("ssh.log"))
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

        fn ssh_log(repo: &Path) -> String {
            std::fs::read_to_string(repo.join("ssh.log")).unwrap_or_default()
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
            let target = repo.path().join("target");
            std::fs::create_dir_all(repo.path().join(".claude")).unwrap();
            std::fs::write(
                repo.path().join(".claude/air.json"),
                serde_json::json!({"targets": {"main": {
                    "kind": "ssh", "host": "box", "user": "me",
                    "path": target.display().to_string(),
                }}})
                .to_string(),
            )
            .unwrap();
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
                log.contains(&format!("git checkout -q --detach {sha}")),
                "{log}"
            );
            assert!(
                !log.contains("git push"),
                "ssh carries receive-pack, not a push command: {log}"
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
}
