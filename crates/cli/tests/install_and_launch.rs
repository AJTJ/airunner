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
    // A known beads prefix, so tmux session names are deterministic (air-5lg): without it the
    // prefix falls back to the temp directory's random name. Committed, because `air record`
    // reports a dirty tree and one test asserts a clean one.
    std::fs::create_dir_all(dir.path().join(".beads")).unwrap();
    std::fs::write(
        dir.path().join(".beads/config.yaml"),
        "issue-prefix: \"zz\"\n",
    )
    .unwrap();
    // `.air/` ignored, or `install --write` refuses (air-6di: the ledger holds messages).
    std::fs::write(dir.path().join(".gitignore"), ".air/\n").unwrap();
    g(&["add", "-A"]);
    g(&["commit", "-q", "-m", "a"]);
    dir
}

/// None of the tests using `air()` are about bd; a missing binary fails in microseconds where
/// a real `bd` in a non-beads directory cost 0.25 to 0.5 s per call (air-4vu, 2026-08-22).
/// Tests that need a bd (doctor, init) build their own fake and set `AIR_BD_BIN` themselves.
fn air(repo: &Path, path_env: Option<&str>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_air"));
    c.arg("--repo")
        .arg(repo)
        .args(args)
        .env("AIR_BD_BIN", "/nonexistent/bd")
        .current_dir(repo);
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
    // This binary first (the on-PATH check), then the system dirs so `git` is reachable for
    // the ignore check (air-6di).
    let path = format!("{}:/usr/bin:/bin", bin_dir.to_string_lossy());
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

/// air-6g1: a repo that already has Air, installed before a surface change, is told what
/// moved under it. A first install is not: nothing has changed under a repo that never had
/// Air. `air install` says it and exits 0 without writing; `--write` records it, and the
/// next run is quiet.
#[test]
fn install_reports_the_surface_diff_to_an_already_installed_repo() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_air")).parent().unwrap();
    // This binary first (the on-PATH check), then the system dirs so `git` is reachable for
    // the ignore check (air-6di).
    let path = format!("{}:/usr/bin:/bin", bin_dir.to_string_lossy());

    // A first install has nothing to report: this repo never had Air.
    let (code, out, _) = air(&repo, Some(&path), &["install"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("SURFACE DIFF"), "{out}");

    let (code, out, err) = air(&repo, Some(&path), &["install", "--write"]);
    assert_eq!(code, 0, "{out}{err}");
    // The version is recorded, so the next run is quiet.
    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join(".air/installed.json")).unwrap())
            .unwrap();
    assert!(!recorded["air_version"].as_str().unwrap().is_empty());
    assert!(
        recorded["surface"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "land")
    );
    let (code, out, _) = air(&repo, Some(&path), &["install"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("SURFACE DIFF"), "{out}");

    // Now the adopter's case: Air is installed, but from before any of this was recorded.
    std::fs::remove_file(repo.join(".air/installed.json")).unwrap();
    let (code, out, _) = air(&repo, Some(&path), &["install"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("SURFACE DIFF"), "{out}");
    assert!(out.contains("air land"), "{out}");
    assert!(out.contains("owner-label"), "{out}");
    // The ones that break a caller without erroring are called out as such.
    assert!(out.contains("WITHOUT erroring"), "{out}");
    assert!(out.contains("dry run"), "{out}");
    // Still a dry run: reporting is not writing.
    assert!(!repo.join(".air/installed.json").exists(), "{out}");

    // --write records it and the diff goes quiet.
    let (code, out, err) = air(&repo, Some(&path), &["install", "--write"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(repo.join(".air/installed.json").exists());
    let (code, out, _) = air(&repo, Some(&path), &["install"]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("SURFACE DIFF"), "{out}");
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
    // air-9dg: the env leads the line as shell assignments, on the process and not only in
    // the blob; `zz` is scratch_repo's prefix.
    assert!(
        out.starts_with(
            "AIR_ROLE=worker BEADS_ACTOR=frontend AIR_ENFORCE=1 AIR_PROJECT=zz claude --append-system-prompt-file "
        ),
        "{out}"
    );
    assert!(
        out.contains("--disallowed-tools 'Bash(air land *)'")
            && out.contains("'Bash(bd create *)'"),
        "{out}"
    );
    // air-0lk: both roles carry the project they may touch.
    assert!(out.contains(r#""AIR_PROJECT":"zz""#), "{out}");
    assert!(out.trim().ends_with("--model opus"), "{out}");
    assert!(repo.join(".air/roles.md").exists());

    // air-jc2p.4: names that read as another role are not a worker's.
    for reserved in ["main", "coordinator", "lane"] {
        let (code, _, err) = air(&repo, None, &["worker", reserved, "--print"]);
        assert_eq!(code, 1, "{reserved}");
        assert!(err.contains("main, coordinator, lane"), "{err}");
    }

    // air-jc2p.2: the lane is a worker launch with its own role and `air land` allowed.
    let (code, out, err) = air(&repo, None, &["lane", "--print"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.starts_with("AIR_ROLE=lane BEADS_ACTOR=lane AIR_ENFORCE=1 AIR_PROJECT=zz claude "),
        "{out}"
    );
    assert!(
        !out.contains("'Bash(air land *)'") && out.contains("'Bash(air close *)'"),
        "{out}"
    );

    let (code, out, _) = air(&repo, None, &["coordinator", "--print"]);
    assert_eq!(code, 0);
    // air-jc2p.1: its own worktree, in the tmux session `<project>-coordinator`, channel attached.
    let wt = repo.join(".claude/worktrees/coordinator");
    for want in [
        "tmux new-session ".to_string(),
        " -s zz-coordinator ".to_string(),
        " -e AIR_ROLE=coordinator -e AIR_PROJECT=zz ".to_string(),
        format!(" -c {} ", wt.display()),
        " -- claude --dangerously-load-development-channels server:air ".to_string(),
        " --name zz-coordinator ".to_string(),
    ] {
        assert!(out.contains(&want), "{want}: {out}");
    }
    assert!(out.contains(r#""AIR_PROJECT":"zz""#), "{out}");
    // `--print` runs nothing: no worktree made.
    assert!(!wt.exists());

    // air-9dg: a pass-through --settings merges into Air's; the line carries one, with both.
    let (code, out, err) = air(
        &repo,
        None,
        &[
            "worker",
            "frontend",
            "--print",
            "--",
            "--settings",
            r#"{"remoteControlAtStartup":false}"#,
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.matches("--settings").count(), 1, "{out}");
    assert!(
        out.contains(r#""remoteControlAtStartup":false"#) && out.contains(r#""AIR_ENFORCE":"1""#),
        "{out}"
    );
    // And a file path is refused, naming what it would have dropped.
    let (code, _, err) = air(
        &repo,
        None,
        &[
            "worker",
            "frontend",
            "--print",
            "--",
            "--settings",
            "s.json",
        ],
    );
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("AIR_ENFORCE") && err.contains("s.json"),
        "{err}"
    );
}

/// air-0lk: `--repo` may not leave this checkout's repository. A worktree of the same repo is
/// fine (that is how the coordinator reads a worker's state); another project is not.
#[test]
fn repo_flag_may_not_point_at_another_project() {
    let a = scratch_repo();
    let b = scratch_repo();
    let (repo_a, repo_b) = (
        a.path().canonicalize().unwrap(),
        b.path().canonicalize().unwrap(),
    );
    // From inside A, pointing at A: fine.
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo_a)
        .args(["holdings"])
        .current_dir(&repo_a)
        .env("AIR_BD_BIN", "/nonexistent/bd")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    // From inside A, pointing at B: refused, naming the rule.
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo_b)
        .args(["holdings"])
        .current_dir(&repo_a)
        .env("AIR_BD_BIN", "/nonexistent/bd")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(
        err.contains("another project") && err.contains("air-0lk"),
        "{err}"
    );
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

#[test]
fn init_gates_then_builds_a_project_from_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let proj = root.join("newproj");
    std::fs::create_dir_all(proj.join("ios/fastlane")).unwrap();
    std::fs::write(proj.join("Makefile"), "deploy-web:\n\techo ship\n").unwrap();
    std::fs::write(proj.join("ios/fastlane/Fastfile"), "").unwrap();
    // Fake bd that supports init (creates .beads) and answers list/show.
    let bd = root.join("bd");
    std::fs::write(&bd, "#!/bin/sh\ncase \"$1\" in --version) echo 'bd version 1.2.2';; init) mkdir -p .beads; echo \"$@\" > .beads/init.args;; config) echo \"$@\" >> .beads/config.args;; show) echo '{\"id\":\"x\",\"status\":\"open\",\"labels\":[]}';; *) echo '[]';; esac\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bd, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_air"))
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let fake_claude = root.join("claude");
    std::fs::write(&fake_claude, "#!/bin/sh\necho '9.9.9 (Claude Code)'\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake_claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path_ok = format!("{bin_dir}:{}:/usr/bin:/bin", root.display());
    let run = |path: &str, args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_air"))
            .arg("--repo")
            .arg(&proj)
            .args(args)
            .env("PATH", path)
            .env("AIR_BD_BIN", &bd)
            .current_dir(&proj)
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    // Gate: no claude on PATH → refused, nothing written.
    let (c, _, err) = run(
        &format!("{bin_dir}:/usr/bin:/bin"),
        &["init", "--write", "--prefix", "np"],
    );
    assert_eq!(c, 2, "{err}");
    assert!(!proj.join(".git").exists() && !proj.join(".beads").exists());
    // Dry run: nothing written.
    let (c, out, _) = run(&path_ok, &["init", "--prefix", "np"]);
    assert_eq!(c, 0, "{out}");
    assert!(
        out.contains("dry run")
            && out.contains("deny Bash(make deploy*)")
            && out.contains("deny Bash(fastlane *)"),
        "{out}"
    );
    assert!(!proj.join(".git").exists());
    // Write: everything appears.
    let (c, out, err) = run(&path_ok, &["init", "--write", "--prefix", "np"]);
    assert_eq!(c, 0, "{out}{err}");
    assert!(proj.join(".git").is_dir());
    assert!(
        std::fs::read_to_string(proj.join(".beads/init.args"))
            .unwrap()
            .contains("--prefix np --non-interactive --init-if-missing --skip-agents --skip-hooks")
    );
    assert_eq!(
        std::fs::read_to_string(proj.join(".gitignore")).unwrap(),
        ".air/\n"
    );
    let aj: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(proj.join(".claude/air.json")).unwrap())
            .unwrap();
    assert!(
        aj["worker_deny"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "Bash(make deploy*)")
    );
    assert!(proj.join("CLAUDE.md").exists());
    assert!(proj.join(".mcp.json").exists() && proj.join(".air/roles.md").exists());
    // air-arq: the roles prose a fresh init writes carries the run-to-completion sentence.
    // air-8zu reworded it off "hand-over", which is one repo's flow rather than Air's fact.
    let roles = std::fs::read_to_string(proj.join(".air/roles.md")).unwrap();
    // air-7q5: the run-to-completion sentence stays, now scoped to a session that HAS work.
    assert!(roles.contains("Once you have work, finishing a bead is not a stop."));
    assert!(roles.contains("Starting a session is not being given work."));
    // air-odv: Air's landing does not re-verify and main never holds an unverified commit.
    // This replaces air-eaw's "the landing verify is not a repeat of the worker's" — there is
    // no landing verify. Both halves are pinned: the claim, and the ABSENCE of the old one,
    // because a doc that still promised a second verify would promise a check nothing runs.
    assert!(
        roles.contains("Air's landing does not re-verify"),
        "{roles}"
    );
    assert!(
        !roles.contains("The landing verify is not a repeat"),
        "roles.md must not promise a landing verify that no longer runs (air-odv)"
    );
    // air-arq: the coordinator's failsafe when the channel is quiet.
    assert!(
        roles.contains("A 5-minute heartbeat runs for the whole round."),
        "{roles}"
    );
    assert!(
        proj.join(".claude/skills/air-decomposition/SKILL.md")
            .exists()
    );
    // air-ha8: the discipline for removing mechanisms ships with the mechanisms, renamed on
    // install like the others so it cannot collide with a repo's own skill.
    let do_less =
        std::fs::read_to_string(proj.join(".claude/skills/air-do-less/SKILL.md")).unwrap();
    assert!(do_less.starts_with("---\nname: air-do-less\n"), "{do_less}");
    assert!(do_less.contains("removal condition"), "{do_less}");
    // And `beads` is deliberately not installed: it documents bd 1.2.1, and Air pins 1.2.2.
    assert!(!proj.join(".claude/skills/air-beads").exists());
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(proj.join(".claude/settings.json")).unwrap())
            .unwrap();
    assert!(settings["hooks"]["PermissionDenied"].is_array());
    assert!(
        out.contains("air record verify"),
        "next steps printed: {out}"
    );
    // Idempotent: a second --write changes nothing the user owns.
    std::fs::write(proj.join("CLAUDE.md"), "# mine\n").unwrap();
    let before = std::fs::read_to_string(proj.join(".claude/air.json")).unwrap();
    let (c, _, err) = run(&path_ok, &["init", "--write", "--prefix", "np"]);
    assert_eq!(c, 0, "{err}");
    assert_eq!(
        std::fs::read_to_string(proj.join("CLAUDE.md")).unwrap(),
        "# mine\n"
    );
    assert_eq!(
        std::fs::read_to_string(proj.join(".claude/air.json")).unwrap(),
        before
    );
}

/// air-tdc: a coordinator launches from a Bash tool (stdin is a socket, not a tty). The
/// launcher must not exec `claude --tmux` there; it starts a detached tmux session instead.
#[test]
fn worker_with_task_and_no_tty_starts_a_detached_tmux_session() {
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("SKIP: tmux not installed; detached launch cannot be exercised");
        return;
    }
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let stub = repo.join("claude-stub.sh");
    std::fs::write(
        &stub,
        "#!/bin/sh\nd=\"$(dirname \"$0\")\"\nprintf '%s\\n' \"$@\" > \"$d/argv.tmp\"\nmv \"$d/argv.tmp\" \"$d/argv.txt\"\nexec sleep 30\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let socket = format!("air-test-{}", std::process::id());
    let tmux = |args: &[&str]| {
        Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap()
    };

    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["worker", "w", "--task", "hello there"])
        .current_dir(&repo)
        .env("AIR_CLAUDE_BIN", &stub)
        .env("AIR_TMUX_SOCKET", &socket)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // air-5lg: the session carries the project prefix, so a machine-wide `tmux ls` says which
    // fleet a pane belongs to.
    let has = tmux(&["has-session", "-t", "zz-w"]).status.success();
    // Poll for the stub's argv file (normally <0.5 s; macOS scans a freshly written
    // executable on first exec, which has taken >2 s); kill the server regardless.
    let argv_path = repo.join("argv.txt");
    let mut argv = String::new();
    for _ in 0..1000 {
        if let Ok(s) = std::fs::read_to_string(&argv_path) {
            argv = s;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    tmux(&["kill-server"]);

    assert_eq!(out.status.code(), Some(0), "{stdout}{stderr}");
    assert!(has, "tmux session zz-w missing: {stdout}{stderr}");
    assert!(stdout.contains("attach -t zz-w"), "{stdout}");
    assert!(!stderr.contains("tcgetattr"), "{stderr}");
    let lines: Vec<&str> = argv.lines().collect();
    // The prompt goes first (air-2ct: after the deny list it reads as one more deny rule),
    // and it names the task file rather than carrying the task (air-er0: the text in argv is
    // what `pkill -f` matched on the adopter, seven workers in a day).
    let task_path = repo.join(".air").join("tasks").join("w.md");
    assert_eq!(
        lines.first().copied(),
        Some(
            format!(
                "Your task is in {}. Read that file and carry it out.",
                task_path.display()
            )
            .as_str()
        ),
        "{argv}"
    );
    assert!(!argv.contains("hello there"), "{argv}");
    assert_eq!(
        std::fs::read_to_string(&task_path).unwrap(),
        "hello there\n"
    );
    assert!(lines.iter().all(|l| !l.starts_with("--tmux")), "{argv}");
    // air-8gj: no `--worktree`; claude was started in the worktree Air made.
    assert!(!lines.contains(&"--worktree"), "{argv}");
    assert_eq!(
        lines.get(1).copied(),
        Some("--append-system-prompt-file"),
        "{argv}"
    );
    assert!(repo.join(".claude/worktrees/w/.git").is_file());
}

#[test]
fn worker_print_with_no_tty_shows_the_tmux_command() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["worker", "w", "--task", "x", "--print"])
        .current_dir(&repo)
        .env("AIR_CLAUDE_BIN", "claude")
        .env_remove("AIR_TMUX_SOCKET")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    // air-9dg: the env rides on `-e`, which is how a pane gets it from a running server.
    assert!(
        stdout.starts_with(
            "tmux new-session -d -s zz-w -e AIR_ROLE=worker -e BEADS_ACTOR=w -e AIR_ENFORCE=1 -e AIR_PROJECT=zz -c "
        ),
        "{stdout}"
    );
    assert!(!stdout.contains("--tmux"), "{stdout}");
}

/// air-5lg: `tmux ls` is machine-wide, so `air status` is where the owner goes from a lane to
/// its pane. The session name shows on the worker's row when a session for it is live.
#[test]
fn status_names_the_workers_tmux_session() {
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("SKIP: tmux not installed");
        return;
    }
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let socket = format!("air-test-status-{}", std::process::id());
    let tmux = |args: &[&str]| {
        Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap()
    };
    let status = || -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_air"))
            .arg("--repo")
            .arg(&repo)
            .arg("status")
            .current_dir(&repo)
            .env("AIR_TMUX_SOCKET", &socket)
            .env("AIR_BD_BIN", "/nonexistent-bd")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    };

    // No server yet: no tmux column, and nothing fails.
    assert!(!status().contains("tmux "), "{}", status());
    // The main checkout's own lane, live.
    tmux(&["new-session", "-d", "-s", "zz-main", "sleep", "30"]);
    let s = status();
    tmux(&["kill-server"]);
    assert!(s.contains("tmux zz-main"), "{s}");
}

/// air-5lg: `air worker` with no name takes the next free `worker-<N>` (air-jc2p.4) rather than refusing, so a
/// coordinator that has no semantically useful name to give does not invent one from the bead
/// (a worker outlives its bead; owner ruling 2026-08-22).
#[test]
fn worker_with_no_name_picks_the_next_free_lane() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["worker", "--task", "x", "--print"])
        .current_dir(&repo)
        .env("AIR_CLAUDE_BIN", "claude")
        .env("AIR_TMUX_SOCKET", "air-test-noname")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stdout}{stderr}");
    assert!(stderr.contains("no name given; using worker-1"), "{stderr}");
    assert!(
        stdout.contains("new-session -d -s zz-worker-1 -e "),
        "{stdout}"
    );
    // air-jc2p.4: the harness's session name is the tmux session's.
    assert!(stdout.contains(" --name zz-worker-1 "), "{stdout}");
    assert!(
        stdout.contains("/.claude/worktrees/worker-1 "),
        "the lane's worktree is claude's cwd: {stdout}"
    );
}
