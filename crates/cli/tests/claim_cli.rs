//! `air claim` / `air release` against a fake `bd` (a shell script that logs its argv and
//! exits as told). Proves the order: ledger refusal first, bd second, ledger row only after
//! bd succeeded, and nothing written when bd refuses.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let g = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(args)
            .env("GIT_AUTHOR_NAME", "air")
            .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
            .env("GIT_COMMITTER_NAME", "air")
            .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    g(&["init", "-q", "-b", "main"]);
    g(&["commit", "-q", "--allow-empty", "-m", "a"]);
    dir
}

/// A fake bd: appends argv to `<dir>/bd.log`; exits 1 when `<dir>/bd.fail` exists.
fn fake_bd(dir: &Path) -> PathBuf {
    let script = dir.join("bd");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$@\" >> {log}\n[ -e {fail} ] && exit 1\nexit 0\n",
            log = dir.join("bd.log").display(),
            fail = dir.join("bd.fail").display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    script
}

fn air(repo: &Path, bd: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(repo)
        .args(args)
        .env("AIR_BD_BIN", bd)
        .env("BEADS_ACTOR", "tester")
        .current_dir(repo)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn claims(repo: &Path) -> Vec<(String, String, Option<String>)> {
    let conn = rusqlite::Connection::open(repo.join(".air/ledger.db")).unwrap();
    let mut st = conn
        .prepare("SELECT bead, worker, release_reason FROM claims ORDER BY bead")
        .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn claim_runs_bd_then_writes_the_row_and_release_reopens() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);

    let (code, out, _) = air(&repo, &bd, &["claim", "fd-1", "--files", "a.rs,b.rs"]);
    assert_eq!(code, 0, "{out}");
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert_eq!(log.trim(), "update fd-1 --claim --actor tester");
    assert_eq!(claims(&repo), vec![("fd-1".into(), "main".into(), None)]);

    let (code, _, err) = air(&repo, &bd, &["release", "fd-1", "--reason", "bogus"]);
    assert_eq!(code, 1);
    assert!(err.contains("--reason must be one of"));

    let (code, out, _) = air(&repo, &bd, &["release", "fd-1", "--reason", "abandoned"]);
    assert_eq!(code, 0, "{out}");
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert!(
        log.lines().nth(1).unwrap().trim() == "update fd-1 -s open",
        "{log}"
    );
    assert_eq!(claims(&repo)[0].2.as_deref(), Some("abandoned"));
}

#[test]
fn bd_refusal_writes_nothing() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    std::fs::write(repo.join("bd.fail"), "").unwrap();

    let (code, out, _) = air(&repo, &bd, &["claim", "fd-2"]);
    assert_eq!(code, 1);
    assert!(
        out.contains("bd refused the claim; nothing recorded"),
        "{out}"
    );
    assert!(claims(&repo).is_empty());
}

#[test]
fn capture_inbox_triage_round_trip() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);

    let (code, out, _) = air(&repo, &bd, &["--json", "capture", "docs wrong in X"]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let id = v["id"].as_str().unwrap().to_string();
    assert_eq!(v["inbox_depth"], 1);

    let (_, out, _) = air(&repo, &bd, &["inbox"]);
    assert!(out.contains("docs wrong in X"));

    let (code, _, err) = air(&repo, &bd, &["triage", &id]);
    assert_eq!(code, 1, "{err}");

    let (code, out, _) = air(&repo, &bd, &["triage", &id, "--bead", "fd-7"]);
    assert_eq!(code, 0, "{out}");
    let (code, _, _) = air(&repo, &bd, &["triage", &id, "--drop", "dup"]);
    assert_eq!(code, 2);
    let (_, out, _) = air(&repo, &bd, &["inbox"]);
    assert_eq!(out.trim(), "inbox empty");
}
