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
    assert!(out.starts_with("claude --channels server:air "), "{out}");
}
