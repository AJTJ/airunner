//! `air install` (dry run, PATH refusal, real --write) and the launchers' `--print`.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::path::Path;
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

fn air(repo: &Path, path_env: Option<&str>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_air"));
    c.arg("--repo").arg(repo).args(args).current_dir(repo);
    if let Some(p) = path_env {
        c.env("PATH", p);
    }
    let out = c.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn install_dry_run_then_refuses_then_writes_idempotently() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(
        repo.join(".claude/settings.json"),
        r#"{"permissions":{"allow":["Bash(ls *)"]}}"#,
    )
    .unwrap();

    // Dry run: nothing written.
    let (code, out, _) = air(&repo, Some("/nonexistent"), &["install"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("dry run"));
    assert!(!repo.join(".mcp.json").exists());

    // --write with the wrong `air` on PATH: refused, nothing written.
    let (code, _, err) = air(&repo, Some("/nonexistent"), &["install", "--write"]);
    assert_eq!(code, 2, "{err}");
    assert!(!repo.join(".mcp.json").exists());

    // --write with PATH resolving to this binary: written, user settings preserved.
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_air")).parent().unwrap();
    let path = bin_dir.to_string_lossy().to_string();
    let (code, out, err) = air(&repo, Some(&path), &["install", "--write"]);
    assert_eq!(code, 0, "{out}{err}");
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join(".claude/settings.json")).unwrap())
            .unwrap();
    assert_eq!(settings["permissions"]["allow"][0], "Bash(ls *)");
    assert_eq!(
        settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        "air hook"
    );
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["air"]["args"][0], "mcp");
    assert!(repo.join(".air/roles.md").exists());
    let skill =
        std::fs::read_to_string(repo.join(".claude/skills/air-decomposition/SKILL.md")).unwrap();
    assert!(
        skill.starts_with("---\nname: air-decomposition\n"),
        "{}",
        &skill[..60]
    );
    assert!(
        repo.join(".claude/skills/air-phase-transitions/SKILL.md")
            .exists()
    );

    // Second --write: no change.
    let before = std::fs::read_to_string(repo.join(".claude/settings.json")).unwrap();
    let (code, out, _) = air(&repo, Some(&path), &["--json", "install", "--write"]);
    assert_eq!(code, 0, "{out}");
    let plan: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(plan["settings_changed"], false);
    assert_eq!(plan["mcp_changed"], false);
    assert_eq!(
        std::fs::read_to_string(repo.join(".claude/settings.json")).unwrap(),
        before
    );
}

#[test]
fn launchers_print_the_exact_command() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let (code, out, err) = air(
        &repo,
        None,
        &["worker", "frontend", "--print", "--", "--model", "opus"],
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("claude --worktree frontend "), "{out}");
    assert!(
        out.contains("--disallowed-tools 'Bash(air land *)'")
            && out.contains("'Bash(bd create *)'"),
        "{out}"
    );
    assert!(out.contains("BEADS_ACTOR"), "{out}");
    assert!(out.trim().ends_with("--model opus"), "{out}");
    assert!(repo.join(".air/roles.md").exists());

    let (code, _, err) = air(&repo, None, &["worker", "main", "--print"]);
    assert_eq!(code, 1);
    assert!(err.contains("not `main`"));

    let (code, out, _) = air(&repo, None, &["coordinator", "--print"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("claude --dangerously-load-development-channels server:air "), "{out}");
}

#[test]
fn repo_deny_rules_are_appended_from_claude_air_json() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(
        repo.join(".claude/air.json"),
        r#"{"worker_deny": ["Bash(make deploy*)"], "coordinator_deny": ["Bash(rm -rf *)"]}"#,
    )
    .unwrap();
    let (_, out, _) = air(&repo, None, &["worker", "w", "--print"]);
    assert!(out.contains("'Bash(make deploy*)'"), "{out}");
    let (_, out, _) = air(&repo, None, &["coordinator", "--print"]);
    assert!(out.contains("'Bash(rm -rf *)'"), "{out}");
    assert!(!out.contains("deploy"), "{out}");
}

#[test]
fn record_keeps_the_command_and_flags_suspicious_and_changed_runs() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    // Refuses backgrounding.
    let (code, _, err) = air(
        &repo,
        None,
        &["record", "verify", "--", "sh", "-c", "true", "&"],
    );
    assert_eq!(code, 1);
    assert!(err.contains("backgrounded"));
    // A silent, instant green is recorded and flagged.
    let (code, _, err) = air(&repo, None, &["record", "verify", "--", "true"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("suspicious"), "{err}");
    // A different command next time is flagged as changed; output is counted.
    let (code, out, err) = air(
        &repo,
        None,
        &["--json", "record", "verify", "--", "sh", "-c", "echo hello"],
    );
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("command-changed"), "{err}");
    assert!(
        out.starts_with("hello\n"),
        "child output is streamed: {out}"
    );
    let json_start = out.find('{').unwrap();
    let run: serde_json::Value = serde_json::from_str(&out[json_start..]).unwrap();
    assert_eq!(run["command"], "sh -c echo hello");
    assert_eq!(run["output_bytes"], 6);
    assert_eq!(run["dirty"], false);
    let conn = rusqlite::Connection::open(repo.join(".air/ledger.db")).unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT count(*) FROM verify_runs WHERE command IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 2);
}

#[test]
fn doctor_gates_on_bd_answering() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    // No bd on PATH: doctor reports and exits 2.
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .arg("doctor")
        .env("AIR_BD_BIN", repo.join("no-such-bd"))
        .current_dir(&repo)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("bd list --json: FAILED"), "{text}");
    // A bd that answers `list --json` with an array: ok, and the version mismatch is named.
    let fake = repo.join("bd");
    std::fs::write(
        &fake,
        "#!/bin/sh\ncase \"$1\" in --version) echo 'bd version 1.2.1';; *) echo '[]';; esac\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["--json", "doctor"])
        .env("AIR_BD_BIN", &fake)
        .current_dir(&repo)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).unwrap();
    assert_eq!(v["bd"]["version"], "1.2.1");
    assert_eq!(v["bd"]["version_ok"], false);
    assert_eq!(v["bd"]["list_count"], 0);
}

#[test]
fn flaky_head_is_reported_by_record_and_handover() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let (c, _, _) = air(
        &repo,
        None,
        &["record", "verify", "--", "sh", "-c", "echo ok"],
    );
    assert_eq!(c, 0);
    let (_, _, err) = air(
        &repo,
        None,
        &["record", "verify", "--", "sh", "-c", "echo boom; exit 1"],
    );
    assert!(err.contains("flaky at HEAD: 1 green / 1 red"), "{err}");
    let (_, out, _) = air(&repo, None, &["--json", "handover"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let m = v["missing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["check"] == "verify-green-at-head")
        .unwrap();
    assert!(m["detail"].as_str().unwrap().contains("flaky"), "{m}");
}
