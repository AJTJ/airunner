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

/// A fake bd: appends argv to `<dir>/bd.log`; `show` answers from `<dir>/bd.issue.json` when
/// it exists, otherwise one open, unassigned, unlabelled issue per id argument, omitting any
/// id listed in `<dir>/bd.unknown` and still exiting 0 (real bd 1.2.2 does exactly that);
/// `list --status in_progress` answers from `<dir>/bd.in_progress` and
/// `list --status awaiting_review` from `<dir>/bd.awaiting_review` (ids, one per line), each
/// carrying the description in `<dir>/bd.desc.json` when that file exists (a JSON string
/// literal, quotes included, so acceptance criteria can be exercised — air-ayp);
/// `update` exits 1 when `<dir>/bd.fail` exists.
///
/// The script is written once per test binary and reads `<dir>` from `FAKE_BD_DIR` (air
/// passes its environment through to bd): macOS charges ~0.5 s on the first exec of every
/// freshly written executable, which was the largest single cost in this file (air-4vu,
/// 2026-08-22). `air()` sets `FAKE_BD_DIR` to the repo.
fn fake_bd(_dir: &Path) -> PathBuf {
    static SCRIPT: std::sync::OnceLock<(tempfile::TempDir, PathBuf)> = std::sync::OnceLock::new();
    SCRIPT
        .get_or_init(|| {
            let home = tempfile::tempdir().unwrap();
            let script = home.path().join("bd");
            std::fs::write(
                &script,
                r#"#!/bin/sh
d="$FAKE_BD_DIR"
echo "$@" >> "$d/bd.log"
case "$1" in
  --version) echo "bd version 1.2.2"; exit 0;;
  show) if [ -f "$d/bd.issue.json" ]; then cat "$d/bd.issue.json"; exit 0; fi
       shift; out=""; desc=""; [ -f "$d/bd.desc.json" ] && desc=$(cat "$d/bd.desc.json")
       for id in "$@"; do
         case "$id" in --*) continue;; esac
         if [ -f "$d/bd.unknown" ] && grep -qx "$id" "$d/bd.unknown"; then continue; fi
         row="{\"id\":\"$id\",\"status\":\"open\",\"labels\":[]"
         [ -n "$desc" ] && row="$row,\"description\":$desc"
         out="$out${out:+,}$row}"
       done
       printf '%s\n' "[$out]"; exit 0;;
  list) f="$d/bd.in_progress"; s=in_progress; case "$*" in *awaiting_review*) f="$d/bd.awaiting_review"; s=awaiting_review;; esac
       desc=""; [ -f "$d/bd.desc.json" ] && desc=$(cat "$d/bd.desc.json")
       out=""
       if [ -f "$f" ]; then
         while IFS= read -r id; do
           [ -n "$id" ] || continue
           row="{\"id\":\"$id\",\"status\":\"$s\""
           [ -n "$desc" ] && row="$row,\"description\":$desc"
           out="$out${out:+,}$row}"
         done < "$f"
       fi
       # printf, not echo: /bin/sh's echo expands the \n inside the JSON description.
       printf '%s\n' "[$out]"; exit 0;;
  ready) echo "[]"; exit 0;;
  update) [ -e "$d/bd.fail" ] && exit 1; exit 0;;
  *) exit 0;;
esac
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

fn air(repo: &Path, bd: &Path, args: &[&str]) -> (i32, String, String) {
    air_env(repo, bd, args, &[])
}

/// `air` with extra environment. The only thing it is used for today is pinning a **dated
/// cutoff** so a test's colour does not depend on the day it runs.
///
/// Two of Air's rules have a date in them — `attribution::FALLBACK_BEFORE` and
/// `handover::FRONTMATTER_SINCE`, both 2026-08-23T00:00:00Z — because each replaced a guess
/// with a declaration and let the old artefacts age out. The tests below build their commits
/// and digests at the CURRENT time, so on 2026-08-22 they were inside the fallback window and
/// on 2026-08-23 they were outside it: seven tests in this file went red six days after they
/// landed green, with no code change in between. Every test here now writes what today's rule
/// wants (a `Bead:` trailer, a `bead:` front-matter line); the one test that is ABOUT a
/// fallback pins its cutoff through this, and that pin is deleted when the fallback is.
fn air_env(repo: &Path, bd: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_air"));
    cmd.arg("--repo")
        .arg(repo)
        .args(args)
        .env("AIR_BD_BIN", bd)
        .env("FAKE_BD_DIR", repo)
        .env("BEADS_ACTOR", "tester")
        .current_dir(repo);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// `air hook` with a hook payload on stdin, the way Claude Code invokes it.
fn air_hook(repo: &Path, bd: &Path, payload: serde_json::Value, enforce: bool) -> (i32, String) {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(repo)
        .arg("hook")
        .env("AIR_BD_BIN", bd)
        .env("FAKE_BD_DIR", repo)
        .env("BEADS_ACTOR", "tester")
        .env("AIR_ENFORCE", if enforce { "1" } else { "0" })
        .current_dir(repo)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut body = payload;
    body["session_id"] = "s1".into();
    body["cwd"] = repo.to_string_lossy().to_string().into();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
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

/// air-p61: the other branch of the same timeout. bd hangs and the write did NOT land, so
/// there is nothing to reconcile.
///
/// The dangerous defect here was never the timeout — it was the sentence. A message that says
/// "nothing was recorded" asserts a state Air cannot know: it stopped waiting, it did not
/// watch bd finish. So the decision must be its own word (`timeout`, never `bd-refused`, which
/// means bd answered and said no), and the message must send the reader to `bd show`.
#[test]
fn a_bd_timeout_is_not_a_refusal_and_does_not_claim_to_know_bd_state() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    // Hangs on `update` and writes nothing; `show` answers at once and knows nothing.
    let slow = repo.join("bd-hang");
    std::fs::write(
        &slow,
        format!(
            "#!/bin/sh\ncase \"$1\" in update) sleep 3; exit 0;; *) exec {bd} \"$@\";; esac\n",
            bd = bd.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // Warm the first exec, which pays a macOS security assessment, so the budget below
    // measures bd and not the OS.
    let _ = Command::new(&slow)
        .arg("show")
        .arg("warm")
        .output()
        .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["claim", "fd-9"])
        .env("AIR_BD_BIN", &slow)
        .env("FAKE_BD_DIR", &repo)
        .env("AIR_BD_TIMEOUT_MS", "1000")
        .env("BEADS_ACTOR", "tester")
        .current_dir(&repo)
        .output()
        .unwrap();
    // The refusal is printed on stdout by `emit`; stderr carries anything else.
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("bd timed out"), "{err}");
    assert!(err.contains("bd's state is unknown"), "{err}");
    assert!(
        err.contains("bd show fd-9"),
        "must send the reader to bd: {err}"
    );
    assert!(
        !err.contains("refused"),
        "a timeout is not a refusal: {err}"
    );
    // Nothing in the ledger: the row is written only after a confirmed result.
    assert!(claims(&repo).is_empty(), "{:?}", claims(&repo));
    // And the event line carries `timeout`, so `air audit` can count how often it fires.
    let events = std::fs::read_dir(repo.join(".air/events"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    assert!(events.contains(r#""decision":"timeout""#), "{events}");
    assert!(!events.contains(r#""decision":"bd-refused""#), "{events}");
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
            .env("FAKE_BD_DIR", &repo)
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

    // A digest written now, after the first claim, declaring the bead it is about (air-agq).
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(repo.join(".claude/air.json"), r#"{"digest_dir":"docs/d"}"#).unwrap();
    std::fs::create_dir_all(repo.join("docs/d")).unwrap();
    std::fs::write(
        repo.join("docs/d/2026-main-fd-9.md"),
        "---\nbead: fd-9\n---\n\ndigest\n",
    )
    .unwrap();

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
    // A resolved capture is re-pointed, not refused (air-76z), and says what it left.
    let (code, out, _) = air(&repo, &bd, &["triage", &id, "--drop", "dup"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("re-pointed from bead fd-7"), "{out}");
    let (_, out, _) = air(&repo, &bd, &["inbox"]);
    assert_eq!(out.trim(), "coordinator queue empty");
}

/// air-869: the incident was ten closes as ten `bd` processes at ~1.4 s each. Ten closes
/// through Air are ONE bd process and one ledger transaction; the event line carries what
/// bd cost, and `air status` reads it back. A worker is refused: the one refusal
/// (hand-over needs green) lives on the worker's path and closing would walk around it.
#[test]
fn ten_closes_are_one_bd_process_and_carry_bd_ms() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let ids: Vec<String> = (1..=10).map(|i| format!("fd-{i}")).collect();
    for id in &ids {
        let (code, out, _) = air(&repo, &bd, &["claim", id]);
        assert_eq!(code, 0, "{out}");
    }
    // Count only what `air close` runs.
    std::fs::write(repo.join("bd.log"), "").unwrap();

    let mut argv: Vec<&str> = vec!["--json", "close"];
    argv.extend(ids.iter().map(String::as_str));
    argv.extend(["--reason", "landed in 63cc0a5"]);
    let (code, out, err) = air(&repo, &bd, &argv);
    assert_eq!(code, 0, "{out}{err}");

    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert_eq!(
        log.lines().filter(|l| !l.trim().is_empty()).count(),
        1,
        "ten closes must be one bd process: {log}"
    );
    assert!(log.contains("close fd-1 "), "{log}");
    assert!(log.contains("fd-10"), "{log}");
    assert!(log.contains("--reason landed in 63cc0a5"), "{log}");

    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["bd_processes"], 1);
    assert_eq!(v["released"].as_array().unwrap().len(), 10);
    assert!(
        claims(&repo)
            .iter()
            .all(|c| c.2.as_deref() == Some("landed"))
    );

    // Every event line that shelled out to bd names what it cost.
    let day = std::fs::read_dir(repo.join(".air/events"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .next()
        .unwrap();
    let events = std::fs::read_to_string(&day).unwrap();
    let close_line = events
        .lines()
        .find(|l| l.contains(r#""command":"close""#))
        .unwrap_or_default();
    assert!(!close_line.is_empty(), "no close event in {events}");
    let e: serde_json::Value = serde_json::from_str(close_line).unwrap();
    assert_eq!(e["bd_calls"], 1, "{close_line}");
    assert!(e["bd_ms"].is_u64(), "{close_line}");

    // ...and `air status` reads the median back out of the log.
    let (code, out, err) = air(&repo, &bd, &["status"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("bd: median"), "{out}");

    // A worker may not close.
    let wt = repo.join("wt-w");
    let g = Command::new("git")
        .args([
            "-C",
            repo.to_str().unwrap(),
            "worktree",
            "add",
            "-q",
            "-b",
            "w",
            wt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(g.status.success(), "{}", String::from_utf8_lossy(&g.stderr));
    std::fs::write(repo.join("bd.log"), "").unwrap();
    let (code, out, _) = air(&wt, &bd, &["close", "fd-1", "--reason", "x"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("coordinator's landing pass"), "{out}");
    assert_eq!(std::fs::read_to_string(repo.join("bd.log")).unwrap(), "");
}

/// air-zlq: `air triage` takes ONE capture. The batch it used to take could not finish
/// verification inside the 5 s probe budget past about three ids, because `bd show` costs
/// about a second per id (measured 2026-08-29: 1 id 1.6 s, 5 ids 9.6 s, 26 ids 27.9 s).
/// Batching saved the bd process, which was never the cost here.
///
/// Three captures are still resolved, one call each, and re-pointing still works (air-76z).
#[test]
fn triage_takes_one_capture_at_a_time() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let mut ids = Vec::new();
    for text in ["one", "two", "three"] {
        let (_, out, _) = air(&repo, &bd, &["--json", "capture", text]);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        ids.push(v["id"].as_str().unwrap().to_string());
    }
    // A second id is not a second capture to triage: clap refuses the extra argument, so
    // there is no batch to half-finish.
    let (code, _, err) = air(&repo, &bd, &["triage", &ids[0], &ids[1], "--bead", "fd-1"]);
    assert_ne!(code, 0, "{err}");
    // Promoted or dropped, never both.
    let (code, _, err) = air(
        &repo,
        &bd,
        &["triage", &ids[0], "--bead", "fd-1", "--drop", "dup"],
    );
    assert_eq!(code, 1, "{err}");

    for (id, args) in [
        (&ids[0], vec!["--bead", "fd-1"]),
        (&ids[1], vec!["--bead", "fd-2"]),
        (&ids[2], vec!["--drop", "dup"]),
    ] {
        let mut argv = vec!["--json", "triage", id.as_str()];
        argv.extend(args);
        let (code, out, err) = air(&repo, &bd, &argv);
        assert_eq!(code, 0, "{out}{err}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["resolved"], 1);
    }
    let (_, out, _) = air(&repo, &bd, &["--json", "triage", &ids[0], "--bead", "fd-9"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["inbox_depth"], 0);
    // A second pass re-points rather than refusing (air-76z).
    assert_eq!(v["repointed"][0]["from"], "bead fd-1");
    assert_eq!(v["repointed"][0]["to"], "bead fd-9");
}

/// air-76z: a capture must never end up pointing at a bead that does not exist. The
/// coordinator chained `air triage C --bead <placeholder>` before `bd create` had made the
/// id, twice, and `air triage` then refused to touch a resolved capture, so the record was
/// wrong and stayed wrong.
#[test]
fn triage_refuses_an_unknown_bead_and_can_repoint_afterwards() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let (_, out, _) = air(&repo, &bd, &["--json", "capture", "needs a bead"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let id = v["id"].as_str().unwrap().to_string();

    // bd does not have this one.
    std::fs::write(repo.join("bd.unknown"), "zz-nope\n").unwrap();
    let (code, out, _) = air(&repo, &bd, &["triage", &id, "--bead", "zz-nope"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("bd knows no bead zz-nope"), "{out}");
    assert!(out.contains("Nothing was triaged"), "{out}");
    // Still open: the refusal wrote nothing.
    let (_, inbox, _) = air(&repo, &bd, &["inbox"]);
    assert!(inbox.contains("needs a bead"), "{inbox}");

    // One bd process checks every bead in the pass, however many.
    std::fs::write(repo.join("bd.log"), "").unwrap();
    let (code, out, _) = air(&repo, &bd, &["triage", &id, "--bead", "ad-real"]);
    assert_eq!(code, 0, "{out}");
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert_eq!(
        log.lines().filter(|l| l.starts_with("show ")).count(),
        1,
        "{log}"
    );

    // And the wrong pointer can be corrected after the fact; the event names both ids.
    let (code, out, _) = air(&repo, &bd, &["--json", "triage", &id, "--bead", "ad-fixed"]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["repointed"][0]["from"], "bead ad-real");
    assert_eq!(v["repointed"][0]["to"], "bead ad-fixed");
    let day = std::fs::read_dir(repo.join(".air/events"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .next()
        .unwrap();
    let events = std::fs::read_to_string(&day).unwrap();
    assert!(
        events.contains("re-pointed from bead ad-real to bead ad-fixed"),
        "{events}"
    );

    // A capture id nothing matches is reported, not silently accepted.
    let (code, out, _) = air(
        &repo,
        &bd,
        &["triage", "01NOSUCHCAPTURE", "--bead", "ad-real"],
    );
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("no such capture"), "{out}");
}

/// air-76z: if bd cannot answer, nothing is triaged. An unverified id in the record is the
/// bug this bead exists for, so silence from bd is a refusal, not a pass.
#[test]
fn triage_refuses_when_bd_does_not_answer() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let (_, out, _) = air(&repo, &bd, &["--json", "capture", "waiting on bd"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let id = v["id"].as_str().unwrap().to_string();

    let slow = repo.join("bd-slow");
    std::fs::write(
        &slow,
        format!(
            "#!/bin/sh\ncase \"$1\" in show) sleep 3; exit 0;; *) exec {bd} \"$@\";; esac\n",
            bd = bd.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // First exec of a freshly written script pays a macOS security assessment; warm it so
    // the short budget below measures the stub, not the OS (air-y8m).
    let _ = Command::new(&slow).arg("ready").output().unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["triage", &id, "--bead", "fd-1"])
        .env("AIR_BD_BIN", &slow)
        .env("FAKE_BD_DIR", &repo)
        .env("AIR_BD_PROBE_TIMEOUT_MS", "1000")
        .env("BEADS_ACTOR", "tester")
        .current_dir(&repo)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(text.contains("bd did not answer"), "{text}");
    assert!(text.contains("nothing was triaged"), "{text}");
    // The capture is untouched, so the coordinator can simply re-run.
    let (_, inbox, _) = air(&repo, &bd, &["inbox"]);
    assert!(inbox.contains("waiting on bd"), "{inbox}");
}

/// air-5hw: the gate is the `owner` label, and `human` is not a gate at all. `human` says a
/// person is present and watching; `owner` says whose authority is required. A worker is
/// refused the first and may claim the second.
#[test]
fn owner_label_is_the_gate_and_human_is_not() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    // A worker, not the coordinator: the label only gates workers.
    let wt = repo.join("wt-w");
    let g = Command::new("git")
        .args([
            "-C",
            repo.to_str().unwrap(),
            "worktree",
            "add",
            "-q",
            "-b",
            "w",
            wt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(g.status.success(), "{}", String::from_utf8_lossy(&g.stderr));

    // The stub answers from FAKE_BD_DIR, which `air()` sets to the directory it is given.
    let says = |dir: &Path, json: &str| std::fs::write(dir.join("bd.issue.json"), json).unwrap();

    says(&wt, r#"{"id":"fd-1","status":"open","labels":["owner"]}"#);
    let (code, out, _) = air(&wt, &bd, &["claim", "fd-1"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("is labelled `owner`"), "{out}");
    assert!(out.contains("air capture --for owner"), "{out}");

    // `human` is presence, not authority: it does not stop a worker.
    says(&wt, r#"{"id":"fd-2","status":"open","labels":["human"]}"#);
    let (code, out, _) = air(&wt, &bd, &["claim", "fd-2"]);
    assert_eq!(code, 0, "{out}");

    // The coordinator is not gated by it either way.
    says(&repo, r#"{"id":"fd-3","status":"open","labels":["owner"]}"#);
    let (code, out, _) = air(&repo, &bd, &["claim", "fd-3"]);
    assert_eq!(code, 0, "{out}");
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
            .env("FAKE_BD_DIR", &repo)
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

/// air-eiv: `air handover` is documented as the way to find what is missing, and it used to
/// increment the very counter `handover-not-green` reads. So the documented diagnostic raised
/// the alarm, and the coordinator chased a worker who was following the docs.
///
/// N direct invocations produce no condition; one hook-path refusal produces one.
#[test]
fn air_handover_is_a_query_and_the_hook_path_is_the_attempt() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let alarms = || {
        let (_, out, _) = air(&repo, &bd, &["--json", "status", "--attention"]);
        eprintln!("ATTENTION: {out}");
        let (_, c, _) = air(&repo, &bd, &["--json", "status"]);
        let v: serde_json::Value = serde_json::from_str(&c).unwrap();
        eprintln!(
            "WORKERS: {}",
            serde_json::to_string(&v["snapshot"]["workers"]).unwrap()
        );
        out.matches("handover-not-green").count()
    };
    // bd keeps holding fd-1 in_progress, so `air status`'s reconcile leaves the claim open and
    // the only thing that can move the counter is a hand-over.
    std::fs::write(repo.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air(&repo, &bd, &["claim", "fd-1"]).0, 0);

    // The diagnostic, run the documented number of times: still nothing to report.
    for _ in 0..3 {
        let (code, out, err) = air(&repo, &bd, &["handover", "--bead", "fd-1"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("would refuse"), "{out}");
    }
    assert_eq!(alarms(), 0, "a query must not raise the alarm");

    // The hook path: an actual `bd close`, refused because there is no green at HEAD. THAT is
    // a hand-over attempt, and it is the one the coordinator should see.
    let (code, err) = air_hook(
        &repo,
        &bd,
        serde_json::json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
        "tool_input": {"command": "bd close fd-1 --reason done"}}),
        true,
    );
    assert_eq!(code, 2, "the gate must refuse: {err}");
    assert_eq!(alarms(), 1, "the hook path must raise it exactly once");
}

/// Digest gate: configured via .claude/air.json; absent → missing; present and newer → pass.
#[test]
fn digest_gate_is_configured_per_repo() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    // A held claim, so the gate has a bead for a digest to declare (air-agq).
    assert_eq!(air(&repo, &bd, &["claim", "fd-3"]).0, 0);
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
    std::fs::write(
        repo.join("docs/log.d/2026-08-21-main-round.md"),
        "---\nbead: fd-3\n---\n\ndigest\n",
    )
    .unwrap();
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

/// air-19u: bd under load took 20 s, the MCP tool budget, so status returned nothing when the
/// fleet was busiest. With a bd that sleeps 25 s, status answers from the ledger in well under
/// 3 s, says bd was slow, keeps sessions and claims, and serves the last cached counts.
#[test]
fn status_answers_fast_from_the_ledger_when_bd_is_slow() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    // A claim and a seeded cache via one healthy status.
    std::fs::write(repo.join("bd.in_progress"), "fd-1\n").unwrap();
    let (code, _, _) = air(&repo, &bd, &["claim", "fd-1"]);
    assert_eq!(code, 0);
    let (code, o, _) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{o}");
    let healthy: serde_json::Value = serde_json::from_str(&o).unwrap();
    let healthy = &healthy["snapshot"];
    assert_eq!(healthy["ready_depth"], 0, "{o}");

    let slow = repo.join("slow-bd");
    // `sleep` runs as a grandchild holding bd's stdout open: killing bd alone must not make
    // status wait for the pipe to close (wait_drained joined its drain threads; air-19u).
    std::fs::write(&slow, "#!/bin/sh\nsleep 25 &\nwait\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let t0 = std::time::Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["--json", "status"])
        .env("AIR_BD_BIN", &slow)
        .env("AIR_BD_TIMEOUT_MS", "500")
        .env("BEADS_ACTOR", "tester")
        .current_dir(&repo)
        .output()
        .unwrap();
    let took = t0.elapsed();
    let o = String::from_utf8_lossy(&out.stdout).to_string();
    assert_eq!(out.status.code(), Some(0), "{o}");
    // 3 s is the acceptance bar with the default 2 s budget; 500 ms here keeps the test fast.
    assert!(
        took < std::time::Duration::from_secs(3),
        "status took {took:?}"
    );
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    let v = &v["snapshot"];
    let errors = v["errors"].to_string();
    assert!(
        errors.contains("bd did not answer in 0.5 s") && errors.contains("stale (last seen "),
        "{errors}"
    );
    assert_eq!(v["ready_depth"], 0, "cached count served: {o}");
    assert_eq!(
        v["awaiting_review"], healthy["awaiting_review"],
        "cached list served: {o}"
    );
    let main = v["workers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["worker"] == "main")
        .unwrap();
    assert_eq!(main["claims"][0]["bead"], "fd-1", "claim kept: {o}");
    assert!(v["duration_ms"].as_u64().unwrap() < 3000, "{o}");
}

/// air-6p5: only the owner may merge to main today, so a green branch waits on them and
/// nothing said so. `air inbox --owner` lists the landings with their exact commands next to
/// the decisions, derived from git plus the ledger rather than stored twice.
///
/// air-7kp: the fixture is a worker BRANCH whose commit names the bead. A coordinator's own
/// checkout is never a landing candidate — there is nothing to merge into main from main.
#[test]
fn owner_queue_lists_green_landings_with_their_commands() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let alpha = repo.join("alpha");
    let g = |cwd: &Path, args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(cwd)
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
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    g(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-alpha",
            alpha.to_str().unwrap(),
        ],
    );

    // Claimed, worked, and committed with a `Bead:` trailer. That commit is the only thing
    // attributing this branch to fd-1.
    std::fs::write(repo.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "fd-1"]).0, 0);
    std::fs::write(repo.join("bd.in_progress"), "").unwrap();
    std::fs::write(alpha.join("work.txt"), "w\n").unwrap();
    g(&alpha, &["add", "work.txt"]);
    g(&alpha, &["commit", "-q", "-m", &bead_trailer("fd-1")]);
    let head = g(&alpha, &["rev-parse", "HEAD"]);

    // Not green at that head yet: the worker's to fix, so the owner is told nothing.
    let (code, out, err) = air(&repo, &bd, &["inbox", "--owner"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("owner queue empty"), "{out}");

    let (code, o, e) = air(&alpha, &bd, &["record", "verify", "--", "true"]);
    assert_eq!(code, 0, "{o}{e}");
    let (code, out, err) = air(&repo, &bd, &["inbox", "--owner"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("1 landing(s) waiting on the owner"), "{out}");
    assert!(
        out.contains(head.get(..8).unwrap()) && out.contains("from alpha"),
        "{out}"
    );
    assert!(out.contains("air land fd-1"), "{out}");

    // And in JSON, next to the captures, so the channel reads one shape.
    let (_, out, _) = air(&repo, &bd, &["--json", "inbox", "--owner"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["landings"][0]["bead"], "fd-1", "{out}");
    assert_eq!(v["landings"][0]["worker"], "alpha", "{out}");
    assert!(v["captures"].as_array().unwrap().is_empty(), "{out}");
}

/// air-6u5: the adopter case end to end, with NO trailer anywhere.
///
/// Claim, commit naming the bead in prose, **merge main**, record the green, land. Merging main
/// is the step that used to destroy the attribution: it moves the branch point forward past
/// the claim, and the old narrowing required `claimed_at >= branch_point`. Landing requires
/// merging main, so preparing to land was what made the branch unlandable — `air land --all`
/// answered `{"landed": [], "ok": true}` with every precondition satisfied.
///
/// This is the one test in the file that is ABOUT the prose fallback, so it is the one that
/// pins `attribution::FALLBACK_BEFORE` (see [`air_env`]) instead of writing a trailer. Delete
/// the pin and the test together when the fallback goes.
#[test]
fn a_branch_that_merged_main_is_still_landable_without_a_trailer() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    // Far enough ahead that every commit this test makes is "before the cutoff", whenever it
    // runs. The rule's own env override, not a second copy of the rule.
    let pin: &[(&str, &str)] = &[("AIR_BEAD_TRAILER_SINCE", "2099-01-01T00:00:00Z")];

    std::fs::write(main.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "fd-1"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    // Prose only: this is a repo with no `Bead:` trailers, which is every repo that has not
    // adopted them yet.
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt"]);
    git(&alpha, &["commit", "-q", "-m", "feat: the work (fd-1)"]);

    // main moves on, and the worker merges it — the ordinary pre-land step.
    std::fs::write(main.join("other.txt"), "other\n").unwrap();
    git(&main, &["add", "other.txt"]);
    git(&main, &["commit", "-q", "-m", "chore: main moves"]);
    git(&alpha, &["merge", "--no-edit", "-q", "main"]);
    // Green LAST, at the merged head.
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);

    // The branch point is now newer than the claim. It must still be landable.
    let (code, out, err) = air_env(&main, &bd, &["--json", "land", "--all"], pin);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ok"], true, "{out}");
    assert_eq!(v["landed"][0], "fd-1", "{out}");
}

/// air-6u5: nothing landable is a REPORT. `ok: true` with an empty `landed` is impossible,
/// because there is nothing in it to disbelieve.
#[test]
fn nothing_landable_names_every_branch_and_its_fix() {
    let (_tmp, main, _alpha) = land_repo("true");
    let bd = fake_bd(&main);

    // alpha exists, is ahead of main, and has no recorded green.
    let (code, out, _) = air(&main, &bd, &["land", "--all"]);
    assert_eq!(code, 2, "not landable must not exit 0: {out}");
    assert!(out.contains("nothing is landable"), "{out}");
    assert!(out.contains("alpha"), "{out}");
    assert!(out.contains("green-at-head"), "{out}");
    assert!(
        out.contains("air record verify"),
        "the fixing command: {out}"
    );

    let (code, out, _) = air(&main, &bd, &["--json", "land", "--all"]);
    assert_eq!(code, 2, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["ok"], false,
        "ok:true with nothing landed is the bug: {out}"
    );
    assert_eq!(v["skipped"][0]["check"], "green-at-head", "{out}");
    assert!(
        !v["skipped"][0]["fix"].as_str().unwrap().is_empty(),
        "{out}"
    );
}

/// air-5lg: `tmux ls` is machine-wide and said nothing about what a lane was doing, so
/// `air claim` renames the worker's tmux window to the bead and `air release` clears it.
#[test]
fn claim_labels_the_tmux_window_and_release_clears_it() {
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("SKIP: tmux not installed");
        return;
    }
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    std::fs::create_dir_all(repo.join(".beads")).unwrap();
    std::fs::write(repo.join(".beads/config.yaml"), "issue-prefix: \"zz\"\n").unwrap();
    let socket = format!("air-test-label-{}", std::process::id());
    let tmux = |args: &[&str]| {
        Command::new("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap()
    };
    let window = || {
        let o = tmux(&["list-windows", "-t", "zz-main", "-F", "#{window_name}"]);
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let air_tmux = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_air"))
            .arg("--repo")
            .arg(&repo)
            .args(args)
            .env("AIR_BD_BIN", &bd)
            .env("FAKE_BD_DIR", &repo)
            .env("BEADS_ACTOR", "tester")
            .env("AIR_TMUX_SOCKET", &socket)
            .current_dir(&repo)
            .output()
            .unwrap()
    };

    tmux(&["new-session", "-d", "-s", "zz-main", "sleep", "30"]);
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"fd-1","title":"the window says what the lane is doing","status":"open","labels":[]}"#,
    )
    .unwrap();
    assert_eq!(air_tmux(&["claim", "fd-1"]).status.code(), Some(0));
    let labelled = window();

    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"fd-1","title":"t","status":"in_progress","assignee":"tester","labels":[]}"#,
    )
    .unwrap();
    let out = air_tmux(&["release", "fd-1", "--reason", "abandoned"]);
    let cleared = window();
    tmux(&["kill-server"]);

    assert_eq!(out.status.code(), Some(0));
    assert!(labelled.starts_with("fd-1 the window says"), "{labelled}");
    assert_eq!(cleared, "main", "release clears the label");
}

/// air-3eu: a bead that briefly visits `awaiting_review` (a stray `bd update`, reverted a
/// minute later) was seen by the next status tick and the reconcile released the ledger claim,
/// leaving the worker "not claimed" while still editing. `awaiting_review` now marks the claim
/// handed over and keeps it; only `closed` releases it.
#[test]
fn awaiting_review_keeps_the_claim_and_close_releases_it() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let row = |bead: &str| -> (String, Option<String>, Option<String>, Option<String>) {
        let conn = rusqlite::Connection::open(repo.join(".air/ledger.db")).unwrap();
        conn.query_row(
            "SELECT claimed_at, first_handover_at, released_at, release_reason \
             FROM claims WHERE bead=?1",
            rusqlite::params![bead],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
    };
    let worker_view = |o: &str| -> serde_json::Value {
        let v: serde_json::Value = serde_json::from_str(o).unwrap();
        v["snapshot"]["workers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["worker"] == "main")
            .unwrap()
            .clone()
    };

    std::fs::write(repo.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air(&repo, &bd, &["claim", "fd-1"]).0, 0);
    let claimed_at = row("fd-1").0;

    // The stray flip: bd holds nothing in_progress and shows the bead in awaiting_review.
    std::fs::write(repo.join("bd.in_progress"), "").unwrap();
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"fd-1","status":"awaiting_review","labels":[]}"#,
    )
    .unwrap();
    let (code, o, _) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{o}");
    let main = worker_view(&o);
    assert_eq!(main["handed_over"][0]["bead"], "fd-1", "still held: {o}");
    assert!(main["claims"].as_array().unwrap().is_empty(), "{o}");
    let (at, handover, released, _) = row("fd-1");
    assert_eq!(at, claimed_at, "original claim time kept");
    assert!(released.is_none(), "the claim must stay open");
    assert!(handover.is_some(), "marked handed over");

    // Reverted: bd holds it in_progress again, and the row is as it was.
    std::fs::write(repo.join("bd.in_progress"), "fd-1\n").unwrap();
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"fd-1","status":"in_progress","assignee":"tester","labels":[]}"#,
    )
    .unwrap();
    let (code, o, _) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{o}");
    let main = worker_view(&o);
    assert_eq!(main["claims"][0]["bead"], "fd-1", "claimed again: {o}");
    assert_eq!(
        row("fd-1"),
        (claimed_at.clone(), handover, None, None),
        "row untouched by the revert"
    );

    // Closed: the claim is released, with `closed` as the reason.
    std::fs::write(repo.join("bd.in_progress"), "").unwrap();
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"fd-1","status":"closed","labels":[]}"#,
    )
    .unwrap();
    let (code, o, _) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{o}");
    let (at, _, released, reason) = row("fd-1");
    assert_eq!(at, claimed_at);
    assert!(released.is_some(), "closed releases the claim");
    assert_eq!(reason.as_deref(), Some("closed"));
    assert!(
        worker_view(&o)["handed_over"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// A main checkout plus one linked worktree `alpha` on `worktree-alpha`, with a bead handed
/// over on it: the shape `air land` lands (air-3pz). `verify` is what the repo's verify
/// command should be (`true` or `false`), committed so main starts clean.
fn land_repo(verify: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let main = tmp.path().join("main");
    let alpha = tmp.path().join("alpha");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(main.join(".claude")).unwrap();
    std::fs::write(
        main.join(".claude/air.json"),
        format!("{{\"verify_command\": \"{verify}\"}}"),
    )
    .unwrap();
    std::fs::write(main.join("README"), "a\n").unwrap();
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "a"]);
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-alpha",
            alpha.to_str().unwrap(),
        ],
    );
    std::fs::write(alpha.join("work.txt"), "the work\n").unwrap();
    // A nested path too, so an acceptance clause naming `docs/note.md` can be discharged
    // against the merge's changed files (air-ayp).
    std::fs::create_dir_all(alpha.join("docs")).unwrap();
    std::fs::write(alpha.join("docs/note.md"), "the note\n").unwrap();
    git(&alpha, &["add", "-A"]);
    git(&alpha, &["commit", "-q", "-m", "the work"]);
    (
        tmp,
        main.canonicalize().unwrap(),
        alpha.canonicalize().unwrap(),
    )
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
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

/// Work `bead` to completion in the `alpha` worktree the way a worker actually does under the
/// owner's 2026-08-22 ruling (air-7kp): claim it, commit work whose message NAMES it, record
/// the green at that head, and close it with proof. No `awaiting_review` anywhere — that is
/// the point. `air land` then has to find the bead from the merge range alone.
fn close_with_proof(main: &Path, alpha: &Path, bd: &Path, bead: &str) {
    std::fs::write(main.join("bd.in_progress"), format!("{bead}\n")).unwrap();
    assert_eq!(air(alpha, bd, &["claim", bead]).0, 0);
    // The commit is what attributes this branch to the bead now, through its `Bead:` trailer.
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    // Only the work: `add -A` would sweep in the stub's own bd.log and conflict at merge.
    git(alpha, &["add", "done.txt"]);
    git(alpha, &["commit", "-q", "-m", &bead_trailer(bead)]);
    // Green last, so it is recorded at the head that carries the commit above.
    assert_eq!(air(alpha, bd, &["record", "verify", "--", "true"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
}

/// The trailer that declares which bead a commit did the work for (air-4re). Guessing it from
/// the message is the fallback, and the fallback expires: see [`air_env`].
fn bead_trailer(bead: &str) -> String {
    format!("feat: the work ({bead})\n\nBead: {bead}\n")
}

/// air-24e, the guard that would have caught this on the day rather than six days later.
///
/// Both fallbacks are pinned DEAD — every artefact this test writes is after the cutoff — and
/// the ordinary claim / commit / green / close / land flow must still work. On 2026-08-22 this
/// would have failed while the suite was green, which is the whole point: it makes the
/// wall clock irrelevant to the suite's verdict instead of waiting for it to expire.
///
/// The pins come from the rules' own env overrides, so when `FALLBACK_BEFORE` and
/// `FRONTMATTER_SINCE` are deleted, the variables go and this test goes with them. A pin
/// written as a second copy of the date would outlive the rule and become the next stale
/// number, which is the defect one level up from the one being fixed here.
#[test]
fn nothing_in_this_suite_leans_on_an_expired_fallback() {
    let dead: &[(&str, &str)] = &[
        ("AIR_BEAD_TRAILER_SINCE", "1970-01-01T00:00:00Z"),
        ("AIR_DIGEST_FRONTMATTER_SINCE", "1970-01-01T00:00:00Z"),
    ];
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);

    // The digest gate on, so both dead fallbacks are in play at once.
    std::fs::write(
        main.join(".claude/air.json"),
        r#"{"verify_command": "true", "digest_dir": "docs/d"}"#,
    )
    .unwrap();
    // Committed: `air land` refuses a dirty main, and rightly.
    git(&main, &["add", ".claude/air.json"]);
    git(&main, &["commit", "-q", "-m", "chore: digest gate on"]);
    git(&alpha, &["merge", "--no-edit", "-q", "main"]);
    std::fs::write(main.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air_env(&alpha, &bd, &["claim", "fd-1"], dead).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    std::fs::create_dir_all(alpha.join("docs/d")).unwrap();
    std::fs::write(
        alpha.join("docs/d/2026-08-29-alpha-fd-1.md"),
        "---\nbead: fd-1\n---\n\ndigest\n",
    )
    .unwrap();
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt", "docs/d"]);
    git(&alpha, &["commit", "-q", "-m", &bead_trailer("fd-1")]);
    assert_eq!(
        air_env(&alpha, &bd, &["record", "verify", "--", "true"], dead).0,
        0
    );

    // The gate passes with no fallback left to lean on: the digest declares its bead.
    let (_, o, _) = air_env(&alpha, &bd, &["--json", "handover"], dead);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["pass"], true, "{o}");

    // And the branch is attributed by its trailer, not by prose.
    let (code, out, err) = air_env(&main, &bd, &["--json", "land", "--all"], dead);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["landed"][0], "fd-1", "{out}");
}

/// The `## Acceptance Criteria` bd returns for every listed bead, as a JSON string literal
/// (air-ayp: bd has no `acceptance_criteria` field, only the description).
fn acceptance(main: &Path, criteria: &str) {
    let body = format!("## Incident\n\nx\n\n## Acceptance Criteria\n\n{criteria}");
    std::fs::write(
        main.join("bd.desc.json"),
        serde_json::to_string(&body).unwrap(),
    )
    .unwrap();
}

/// air-bxe: the landings row exists from the MERGE onward, not from the exit.
///
/// adopter's coordinator reported a land done three times before the process exited, because
/// the merge commit appears minutes before the verify finishes with the rollback armed. Their
/// fallback was `pgrep`, which misled them twice. Separately a land killed by a closed pipe
/// (`air land | head`) merged, verified and wrote nothing, leaving main green at a sha no
/// landing mentioned.
///
/// The observation is made from INSIDE the window: the repo's verify command copies `.air/`
/// aside while the landing is armed, and the test reads that copy. Asserting on the ledger
/// after `air land` returns could never distinguish "written at merge time" from "written at
/// exit", which is the whole of the bead.
///
/// This test declares its bead with a `Bead:` trailer rather than relying on the pre-2026-08-23
/// prose fallback, so it does not share the wall-clock failure air-24e is about.
#[test]
fn a_landing_is_recorded_in_flight_while_the_rollback_is_armed() {
    let (_tmp, main, alpha) = land_repo("sh peek.sh");
    // The repo's "verify": snapshot the ledger mid-land, then pass. Copying the whole `.air`
    // directory takes the WAL sidecars with it, so the copy sees the same rows the ledger does.
    std::fs::write(
        main.join("peek.sh"),
        "#!/bin/sh\nrm -rf seen_air\ncp -R .air seen_air\nexit 0\n",
    )
    .unwrap();
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "peek"]);

    let bd = fake_bd(&main);
    std::fs::write(main.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "fd-1"]).0, 0);
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: fd-1\n"],
    );
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    acceptance(&main, "- Verify recorded green at HEAD.\n");

    let (code, out, err) = air(&main, &bd, &["land", "fd-1"]);
    assert_eq!(code, 0, "{out}{err}");
    let head = git(&main, &["rev-parse", "HEAD"]);

    // What the ledger said WHILE the merge sat in main with the rollback armed.
    let seen = rusqlite::Connection::open(main.join("seen_air/ledger.db")).unwrap();
    let mid: Option<(String, String, String, i64)> = seen
        .query_row(
            "SELECT result, merge_commit, tip_sha, pid FROM landings",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();
    assert!(
        mid.is_some(),
        "a landings row must exist DURING the land, not only after it"
    );
    let (result, merge, tip, pid) = mid.unwrap();
    assert_eq!(result, "in-flight");
    assert_eq!(
        merge, head,
        "and it names the merge that is sitting in main"
    );
    assert_ne!(tip, head, "with the sha a rewind would return to");
    assert!(pid > 0, "and the process to ask about, so nobody greps");

    // Afterwards it is the SAME row, carrying the outcome: one attempt, not two.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let rows: Vec<(String, i64)> = conn
        .prepare("SELECT result, attempt_no FROM landings")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows, vec![("landed".to_string(), 1)]);
}

/// air-03w: the `landable` condition is what `air land --all` selects on.
///
/// Since air-7o3 the worker closes its own bead with proof and never sets `awaiting_review`,
/// so `review-waiting` reports a state this repo stopped using and nothing told the coordinator
/// a branch was ready — it learned by polling `air status`.
///
/// The probe in `air selftest` covers the condition's shape over a hand-built snapshot. This
/// covers the WIRING, which that probe cannot see: `gather` filling `landable` from the same
/// `select` the command runs. Without it, `landable: Vec::new()` in `gather` leaves every
/// selftest probe green while the condition never fires against a real repo.
#[test]
fn a_landable_branch_is_a_condition_and_the_command_agrees() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);

    // Nothing to land yet: silent.
    let (_c, out, err) = air(&main, &bd, &["status", "--attention"]);
    assert!(!out.contains("landable"), "{out}{err}");

    std::fs::write(main.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "fd-1"]).0, 0);
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: fd-1\n"],
    );
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    // Green LAST, so it sits at a head containing main. That transition is the whole subject.
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    let (_c, out, err) = air(&main, &bd, &["status", "--attention"]);
    assert!(out.contains("landable"), "{out}{err}");
    assert!(out.contains("alpha") && out.contains("fd-1"), "{out}");
    assert!(out.contains("air land --all"), "{out}");

    // And the command agrees: what the condition named is what `air land` takes.
    let (code, out, err) = air(&main, &bd, &["land", "--all"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("landed alpha (fd-1)"), "{out}{err}");

    // Landed, so the condition clears itself — the branch is now an ancestor of main.
    let (_c, out, _e) = air(&main, &bd, &["status", "--attention"]);
    assert!(!out.contains("landable"), "{out}");
}

/// air-ob0: a rewind names every worktree that took the un-landed commits.
///
/// adopter, 2026-08-23: *"A rollback un-lands a branch from main but cannot un-merge it from
/// anyone who took it."* A worker who merged main during the armed window — the documented
/// thing to do when main moves — keeps the rewound commits: a recorded green for a tree main
/// will never have, with `air handover` passing and `air land` merging it straight back. So the
/// window is not unverified code in main; it is unverified code that has already propagated, to
/// exactly the workers following the rule.
///
/// The window is real here, not simulated: the repo's verify command IS beta merging main, and
/// it exits red, so the merge beta took is the one main then resets away from. gamma exists and
/// merges nothing, so the test can tell "named everyone" from "named the right one".
#[test]
fn a_rewind_names_the_worktrees_that_took_the_un_landed_commits() {
    let (_tmp, main, alpha) = land_repo("sh window.sh");
    let root = main.parent().unwrap().to_path_buf();
    for name in ["beta", "gamma"] {
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &format!("worktree-{name}"),
                root.join(name).to_str().unwrap(),
            ],
        );
    }
    // beta merges main from INSIDE the armed window and keeps working, then the verify fails.
    // The extra commit matters: it puts beta's HEAD *past* the merge rather than on it, so the
    // carrier check has to answer containment. Without it, equality alone would pass and the
    // test would be blind to the case it exists for.
    let beta = root.join("beta");
    std::fs::write(
        main.join("window.sh"),
        format!(
            "#!/bin/sh\nexport GIT_AUTHOR_NAME=air GIT_AUTHOR_EMAIL=air@x \
             GIT_COMMITTER_NAME=air GIT_COMMITTER_EMAIL=air@x\n\
             git -C {b} merge -q --no-edit main\n\
             echo more > {b}/beta.txt\n\
             git -C {b} add beta.txt\n\
             git -C {b} commit -q -m 'beta keeps working'\n\
             exit 1\n",
            b = beta.display()
        ),
    )
    .unwrap();
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "window"]);

    let bd = fake_bd(&main);
    std::fs::write(main.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "fd-1"]).0, 0);
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: fd-1\n"],
    );
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    let before = git(&main, &["rev-parse", "HEAD"]);
    let (code, out, err) = air(&main, &bd, &["land", "fd-1"]);
    assert_eq!(code, 1, "the land must go red: {out}{err}");
    assert_eq!(git(&main, &["rev-parse", "HEAD"]), before, "main rewound");

    // The rewind message names beta and only beta.
    assert!(out.contains("already in beta"), "{out}{err}");
    assert!(!out.contains("gamma"), "gamma merged nothing: {out}");
    assert!(
        !out.contains("already in main") && !out.contains("in main, beta"),
        "the checkout that was just reset is not a carrier: {out}"
    );

    // ...and the message is not the only copy: `air status` holds the same set afterwards.
    // Read the carried line only: every worktree appears in the status table by definition,
    // so `st.contains("gamma")` would prove nothing either way.
    let carried = |repo: &Path| -> String {
        air(repo, &bd, &["status"])
            .1
            .lines()
            .filter(|l| l.starts_with("rewound and still carried"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let line = carried(&main);
    assert!(line.contains("beta"), "{line}");
    assert!(!line.contains("gamma"), "{line}");

    // It clears itself when nobody carries the commits any more. No expiry to choose.
    git(&beta, &["reset", "--hard", &before]);
    assert!(carried(&main).is_empty(), "{}", carried(&main));
}

/// air-3pz: the coordinator merges a green hand-over, verifies the *merged* result, closes the
/// bead in one bd process, releases the claim, and records the landing.
#[test]
fn land_merges_verifies_closes_and_records() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    close_with_proof(&main, &alpha, &bd, "fd-1");
    // air-ayp: acceptance Air can point at evidence for — a green at the landed sha, and a
    // file the merge changed. Anything else would land merged-but-not-closed.
    acceptance(
        &main,
        "- Verify recorded green at HEAD.\n- docs/note.md carries the note.\n",
    );
    let before = git(&main, &["rev-parse", "HEAD"]);

    let (code, out, err) = air(&main, &bd, &["land", "fd-1"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("landed alpha (fd-1)"), "{out}{err}");

    // main moved to a merge commit that contains the branch.
    let head = git(&main, &["rev-parse", "HEAD"]);
    assert_ne!(head, before);
    assert_eq!(git(&main, &["rev-list", "--count", "HEAD^2..HEAD^2"]), "0");
    assert_eq!(
        std::fs::read_to_string(main.join("work.txt")).unwrap(),
        "the work\n"
    );
    // Every clause discharged, and still nothing is closed: the worker closes its own bead
    // with proof before the branch lands (air-ayp).
    assert!(out.contains("fd-1 — every clause discharged"), "{out}{err}");
    let log = std::fs::read_to_string(main.join("bd.log")).unwrap();
    assert!(!log.contains("close fd-1"), "{log}");
    // And the landing is a row, with the verify run that decided it.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let (worker, result, merge, verify): (String, String, String, String) = conn
        .query_row(
            "SELECT worker, result, merge_commit, verify_run_id FROM landings",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!((worker.as_str(), result.as_str()), ("alpha", "landed"));
    assert_eq!(merge, head);
    let (sha, exit): (String, i64) = conn
        .query_row(
            "SELECT sha, exit_code FROM verify_runs WHERE id=?1",
            rusqlite::params![verify],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (sha.as_str(), exit),
        (head.as_str(), 0),
        "the green is at the MERGED result, not the branch"
    );
}

/// air-ayp: `air land` closes nothing — the worker closes its own bead with proof (owner
/// ruling, 2026-08-22). The landing PRINTS every bead beside its acceptance and Air's verdict
/// per clause, which is the only external check on that honour system. A clause the merge
/// CONTRADICTS is a wrong close: kept on the `landings` row and named by `air status`.
#[test]
fn land_prints_acceptance_closes_nothing_and_flags_a_refuted_clause() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    close_with_proof(&main, &alpha, &bd, "fd-1");
    // Three clauses: one Air can look up, one it can look up and refute, one it cannot read.
    acceptance(
        &main,
        "- Verify recorded green at HEAD.\n\
         - docs/absent.md says the rule.\n\
         - The owner rules on the counter-argument.\n",
    );
    let before = git(&main, &["rev-parse", "HEAD"]);

    let (code, out, err) = air(&main, &bd, &["land", "fd-1"]);
    assert_eq!(code, 0, "{out}{err}");
    // It MERGED: the code is in main, so nothing is held hostage to the prose.
    assert_ne!(git(&main, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(main.join("work.txt")).unwrap(),
        "the work\n"
    );
    // The print: every clause with its verdict, so a wrong close is visible as it lands.
    assert!(out.contains("air land closes nothing"), "{out}");
    assert!(out.contains("ok   Verify recorded green at HEAD."), "{out}");
    assert!(out.contains("MISS docs/absent.md says the rule."), "{out}");
    assert!(
        out.contains("?    The owner rules on the counter-argument."),
        "{out}"
    );
    assert!(out.contains("fd-1 — REFUTED"), "{out}");
    // It closes NOTHING, and writes no bd status of any kind.
    let log = std::fs::read_to_string(main.join("bd.log")).unwrap();
    assert!(!log.contains("close fd-1"), "{log}");
    assert!(
        !log.lines().any(|l| l.starts_with("update fd-1 -s")),
        "no bd status is written, so the bead blocks exactly what it blocked before: {log}"
    );
    // What the print said outlives the scrollback, on the landings row.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let (result, open): (String, String) = conn
        .query_row("SELECT result, open_beads FROM landings", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(result, "landed-refuted");
    assert!(
        open.contains("fd-1") && open.contains("docs/absent.md"),
        "{open}"
    );

    // Restored by air-dlw. Beta weakened this to `!s.is_empty()` when close-with-proof made it
    // fail, with the reason beside it: the condition used to clear on a RELEASED claim row,
    // and under the new flow the worker closes immediately so the reconcile releases the claim
    // on the next tick. The report is derived from the landing and the acceptance verdict now,
    // so the claim being gone says nothing — which is the whole point of air-ayp surviving the
    // flow change.
    let (_, s, _) = air(&main, &bd, &["status"]);
    assert!(
        s.contains("fd-1 landed in") && s.contains("CONTRADICTS"),
        "{s}"
    );
    // The claim really is reconciled away by now, so this is not passing by accident.
    let released: i64 = rusqlite::Connection::open(main.join(".air/ledger.db"))
        .unwrap()
        .query_row(
            "SELECT count(*) FROM claims WHERE bead='fd-1' AND released_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(released, 1, "the claim is gone and the report survives it");

    // Somebody reopens the bead: that is what dealing with it looks like, and it clears.
    std::fs::write(main.join("bd.in_progress"), "fd-1\n").unwrap();
    let (_, s, _) = air(&main, &bd, &["status"]);
    assert!(!s.contains("fd-1 landed in"), "cleared once reopened: {s}");
}

/// air-3pz: a red verify on the merged result puts main back exactly where it was and leaves
/// the branch alone; a dirty main is refused before anything is merged; a branch that does not
/// contain main is refused with the command that fixes it.
#[test]
fn land_rewinds_on_red_and_refuses_dirty_main_or_a_stale_branch() {
    let (_tmp, main, alpha) = land_repo("false");
    let bd = fake_bd(&main);
    close_with_proof(&main, &alpha, &bd, "fd-1");
    let before = git(&main, &["rev-parse", "HEAD"]);
    let branch_head = git(&main, &["rev-parse", "worktree-alpha"]);

    // Dirty main: refused before any merge, because the rewind would discard it. Tracked, on
    // purpose: `git reset --hard` leaves untracked files alone, so they are not at risk.
    std::fs::write(main.join("README"), "edited by the owner\n").unwrap();
    let (code, out, err) = air(&main, &bd, &["land", "fd-1"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(out.contains("git reset --hard"), "{out}");
    assert!(out.contains("README"), "{out}");
    assert_eq!(git(&main, &["rev-parse", "HEAD"]), before, "nothing merged");
    std::fs::write(main.join("README"), "a\n").unwrap();

    // Red on the merged result: main goes back to where it was, the branch is untouched.
    let (code, out, err) = air(&main, &bd, &["land", "fd-1"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("main is back at"), "{out}");
    assert_eq!(git(&main, &["rev-parse", "HEAD"]), before);
    assert_eq!(git(&main, &["rev-parse", "worktree-alpha"]), branch_head);
    // Absent is fine and is itself the point: since air-7kp, selection reads git and the
    // ledger, so a land that fails before merging never shells out to bd at all.
    let log = std::fs::read_to_string(main.join("bd.log")).unwrap_or_default();
    assert!(!log.contains("close fd-1"), "nothing closed: {log}");
    // Every attempt is a row, refusals included, with attempt_no counting up.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let mut st = conn
        .prepare("SELECT result, attempt_no FROM landings ORDER BY attempt_no")
        .unwrap();
    let rows: Vec<(String, i64)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![("refused".to_string(), 1), ("rewound".to_string(), 2)]
    );

    // main moves on: the branch no longer contains it, and the recorded green is not a green
    // of what would land.
    git(&main, &["commit", "-q", "--allow-empty", "-m", "b"]);
    let (code, out, err) = air(&main, &bd, &["land", "fd-1"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        out.contains("does not contain main")
            && out.contains("git merge main && air record verify"),
        "{out}"
    );
}

/// air-3pz: a worker may not land. The coordinator's deny list keeps `git commit` off main,
/// so this is the one allowed path onto it and it must not be a worker's.
#[test]
fn land_refuses_a_worker_and_an_unlandable_bead() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    close_with_proof(&main, &alpha, &bd, "fd-1");

    let (code, out, _) = air(&alpha, &bd, &["land", "fd-1"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("air handover"), "{out}");

    let (code, out, _) = air(&main, &bd, &["land", "fd-9"]);
    assert_eq!(code, 2, "{out}");
    // air-7kp: what is landable comes from the merge range now, so the refusal points at
    // `air status` rather than the owner queue.
    assert!(out.contains("no green branch names fd-9"), "{out}");
    assert!(out.contains("air status"), "{out}");
}
