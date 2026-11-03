//! `air ready` (air-m8u): the coordinator's offer of its head to the lane, recorded in the
//! ledger's `offers` table; refused to workers and the lane by role.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::path::Path;

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

fn air(repo: &Path, role: Option<&str>, args: &[&str]) -> (i32, String, String) {
    let mut cmd = common::command(env!("CARGO_BIN_EXE_air"));
    cmd.arg("--repo").arg(repo).args(args).current_dir(repo);
    if let Some(r) = role {
        cmd.env("AIR_ROLE", r);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn the_coordinator_offers_its_head_and_a_worker_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("note"), "a\n").unwrap();
    git(repo, &["add", "note"]);
    git(repo, &["commit", "-q", "-m", "a"]);
    let sha = git(repo, &["rev-parse", "HEAD"]);

    let (code, _, err) = air(repo, Some("worker"), &["ready"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("coordinator's"), "{err}");
    let (code, _, err) = air(repo, Some("lane"), &["ready"]);
    assert_eq!(code, 2, "{err}");
    let ledger = air_ledger::Ledger::open_in(&repo.join(".air")).unwrap();
    assert!(
        !ledger.offered("main", &sha).unwrap(),
        "a refusal records nothing"
    );

    let (code, out, err) = air(repo, Some("coordinator"), &["ready"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("offered main at") && out.contains("to the lane"),
        "{out}"
    );
    assert!(
        ledger.offered("main", &sha).unwrap(),
        "the offer names this head"
    );

    // A later commit is a new head and needs its own offer; --json prints the row.
    std::fs::write(repo.join("note"), "b\n").unwrap();
    git(repo, &["commit", "-q", "-a", "-m", "b"]);
    let sha2 = git(repo, &["rev-parse", "HEAD"]);
    assert!(!ledger.offered("main", &sha2).unwrap());
    let (code, out, _) = air(repo, None, &["--json", "ready"]);
    assert_eq!(code, 0, "the owner's shell may offer too");
    let row: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(row["sha"], sha2);
    assert_eq!(row["coordinator_autobatch"], false);
    assert!(ledger.offered("main", &sha2).unwrap());

    // With the key true the command says every commit is batch-ready already.
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(
        repo.join(".claude/air.json"),
        r#"{"coordinator_autobatch": true}"#,
    )
    .unwrap();
    let (code, out, _) = air(repo, Some("coordinator"), &["ready"]);
    assert_eq!(code, 0);
    assert!(out.contains("coordinator_autobatch"), "{out}");
}
