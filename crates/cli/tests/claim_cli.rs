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

/// A fake bd: appends argv to `<dir>/bd.log`; `show` answers from `<dir>/bd.issue.json`
/// (default: open, unassigned, no labels); `list --status in_progress` answers from
/// `<dir>/bd.in_progress` (ids, one per line); `update` exits 1 when `<dir>/bd.fail` exists.
fn fake_bd(dir: &Path) -> PathBuf {
    let script = dir.join("bd");
    std::fs::write(
        &script,
        format!(
            r#"#!/bin/sh
echo "$@" >> {log}
case "$1" in
  --version) echo "bd version 1.2.2"; exit 0;;
  show) if [ -f {issue} ]; then cat {issue}; else echo '{{"id":"'"$2"'","status":"open","labels":[]}}'; fi; exit 0;;
  list) if [ -f {inprog} ]; then awk '{{printf "%s{{\"id\":\"%s\",\"status\":\"in_progress\"}}", (NR>1?",":""), $0}} BEGIN{{printf "["}} END{{print "]"}}' {inprog}; else echo "[]"; fi; exit 0;;
  ready) echo "[]"; exit 0;;
  update) [ -e {fail} ] && exit 1; exit 0;;
  *) exit 0;;
esac
"#,
            log = dir.join("bd.log").display(),
            issue = dir.join("bd.issue.json").display(),
            inprog = dir.join("bd.in_progress").display(),
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
    assert!(log.contains("update fd-1 --claim --actor tester"), "{log}");
    // bd now holds it in_progress by this actor.
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"fd-1","status":"in_progress","assignee":"tester","labels":[]}"#,
    )
    .unwrap();
    std::fs::write(repo.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(claims(&repo), vec![("fd-1".into(), "main".into(), None)]);

    let (code, _, err) = air(&repo, &bd, &["release", "fd-1", "--reason", "bogus"]);
    assert_eq!(code, 1);
    assert!(err.contains("--reason must be one of"));

    let (code, out, _) = air(&repo, &bd, &["release", "fd-1", "--reason", "abandoned"]);
    assert_eq!(code, 0, "{out}");
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert!(
        log.lines().any(|l| l.trim() == "update fd-1 -s open"),
        "{log}"
    );
    assert_eq!(claims(&repo)[0].2.as_deref(), Some("abandoned"));
}

/// air-y8m: bd's write lands but bd answers after Air's timeout. The claim is reconciled
/// and recorded at the time it was issued (claimed-late); a re-claim keeps that time; a
/// digest written between the two satisfies the hand-over check.
#[test]
fn slow_bd_claim_is_reconciled_and_reclaim_keeps_the_first_time() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    // `update --claim` writes the issue as in_progress by the actor, then hangs past the
    // timeout; `show` answers at once.
    let slow = repo.join("bd-slow");
    std::fs::write(
        &slow,
        format!(
            "#!/bin/sh\ncase \"$1\" in update) printf '%s' '{{\"id\":\"fd-9\",\"status\":\"in_progress\",\"assignee\":\"tester\",\"labels\":[],\"updated_at\":\"2020-01-01T00:00:00Z\"}}' > {issue}; sleep 3; exit 0;; *) exec {bd} \"$@\";; esac\n",
            issue = repo.join("bd.issue.json").display(),
            bd = bd.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // First exec of a freshly written script pays a macOS security assessment (seen >1.5 s
    // under load, 2026-08-22); warm it so the short timeout below measures bd, not the OS.
    let _ = Command::new(&slow)
        .arg("show")
        .arg("warm")
        .output()
        .unwrap();
    let run = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_air"))
            .arg("--repo")
            .arg(&repo)
            .args(args)
            .env("AIR_BD_BIN", &slow)
            .env("AIR_BD_TIMEOUT_MS", "1500")
            .env("BEADS_ACTOR", "tester")
            .current_dir(&repo)
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let (code, out, err) = run(&["--json", "claim", "fd-9"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("confirms the claim landed"), "{err}");
    assert!(!out.contains("nothing was written"), "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let first = v["claimed_at"].as_str().unwrap().to_string();
    assert_eq!(claims(&repo), vec![("fd-9".into(), "main".into(), None)]);

    // A digest written now, after the first claim.
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(repo.join(".claude/air.json"), r#"{"digest_dir":"docs/d"}"#).unwrap();
    std::fs::create_dir_all(repo.join("docs/d")).unwrap();
    std::fs::write(repo.join("docs/d/2026-main-fd-9.md"), "digest").unwrap();

    // Re-claim: bd already holds it by us; no bd write, the row keeps the first time.
    let (code, out, _) = run(&["--json", "claim", "fd-9"]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["reclaimed"], true);
    assert_eq!(v["claimed_at"].as_str().unwrap(), first);
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert_eq!(
        log.matches("--claim").count(),
        0,
        "re-claim must not write to bd: {log}"
    );

    let (_, o, _) = run(&["--json", "handover"]);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert!(
        !v["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["check"] == "digest-present"),
        "{o}"
    );
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
    assert_eq!(out.trim(), "coordinator queue empty");
}

/// Two worktrees contend for one resource; the dead-holder path is exercised by pointing
/// the holder's pid at a process that has already exited.
#[test]
fn lease_take_deny_break_across_worktrees_and_owner_queue() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    // Second worktree.
    let wt = repo.join("wt-b");
    let out = Command::new("git")
        .args([
            "-C",
            repo.to_str().unwrap(),
            "worktree",
            "add",
            "-q",
            "-b",
            "b",
            wt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // A pid that is certainly dead: spawn `true` and wait for it.
    let dead = Command::new("true").spawn().unwrap();
    let dead_pid = dead.id().to_string();
    let _ = dead.wait_with_output();

    let run = |cwd: &Path, pid: &str, args: &[&str]| -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_air"))
            .arg("--repo")
            .arg(cwd)
            .args(args)
            .env("AIR_BD_BIN", &bd)
            .env("AIR_LEASE_PID", pid)
            .current_dir(cwd)
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
        )
    };
    let me = std::process::id().to_string();

    // main takes runtime with a live pid (this test process); b is denied.
    let (c, o) = run(&repo, &me, &["lease", "take", "--reason", "api"]);
    assert_eq!(c, 0, "{o}");
    let (c, o) = run(&wt, &me, &["lease", "take", "--reason", "sim"]);
    assert_eq!(c, 1);
    assert!(o.contains("HELD by main"), "{o}");
    let (_, o) = run(&repo, &me, &["lease", "status"]);
    assert!(o.contains("wanted by") && o.contains("wt-b"), "{o}");
    // Healthy lease cannot be broken without --force.
    let (c, _) = run(&wt, &me, &["lease", "break"]);
    assert_eq!(c, 1);
    // Re-take by main with a dead pid recorded; b now breaks it and takes it.
    let (c, _) = run(&repo, &me, &["lease", "release"]);
    assert_eq!(c, 0);
    let (c, _) = run(&repo, &dead_pid, &["lease", "take", "--reason", "api"]);
    assert_eq!(c, 0);
    let (_, o) = run(&repo, &me, &["--json", "status", "--attention"]);
    assert!(o.contains("lease-held-by-dead-session"), "{o}");
    let (c, o) = run(&wt, &me, &["lease", "take", "--reason", "sim"]);
    assert_eq!(c, 0, "{o}");
    assert!(o.contains("was dead"), "{o}");
    let (_, o) = run(&repo, &me, &["--json", "lease", "status"]);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v[0]["holder"], "wt-b");

    // Owner queue: separate audience, separate attention condition.
    let (c, _) = run(&wt, &me, &["capture", "--for", "owner", "rule on ports"]);
    assert_eq!(c, 0);
    let (_, o) = run(&repo, &me, &["inbox"]);
    assert!(o.contains("coordinator queue empty"), "{o}");
    let (_, o) = run(&repo, &me, &["inbox", "--owner"]);
    assert!(o.contains("rule on ports"), "{o}");
    let (_, o) = run(&repo, &me, &["--json", "status", "--attention"]);
    assert!(o.contains("owner-decision-waiting"), "{o}");
}

/// Digest gate: configured via .claude/air.json; absent → missing; present and newer → pass.
#[test]
fn digest_gate_is_configured_per_repo() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let (_, o, _) = air(&repo, &bd, &["--json", "handover"]);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert!(
        !v["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["check"] == "digest-present"),
        "{o}"
    );
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(
        repo.join(".claude/air.json"),
        r#"{"digest_dir":"docs/log.d"}"#,
    )
    .unwrap();
    let (_, o, _) = air(&repo, &bd, &["--json", "handover"]);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert!(
        v["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["check"] == "digest-present"),
        "{o}"
    );
    std::fs::create_dir_all(repo.join("docs/log.d")).unwrap();
    std::fs::write(repo.join("docs/log.d/2026-08-21-main-round.md"), "digest").unwrap();
    let (_, o, _) = air(&repo, &bd, &["--json", "handover"]);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert!(
        !v["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["check"] == "digest-present"),
        "{o}"
    );
}
