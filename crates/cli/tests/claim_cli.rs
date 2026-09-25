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
# air-bxe/air-odv: a hook for observing the ledger from INSIDE a command. `air land` calls bd
# for acceptance after main has moved and before it records the outcome, so a snapshot taken
# here is what the ledger said mid-land. Off unless the test creates bd.peek.
[ -f "$d/bd.peek" ] && { rm -rf "$d/seen_air"; cp -R "$d/.air" "$d/seen_air"; }
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
  ready) if [ -f "$d/bd.ready.json" ]; then cat "$d/bd.ready.json"; exit 0; fi; echo "[]"; exit 0;;
  update) [ -e "$d/bd.fail" ] && exit 1
          # air-0kk: `-s open -a ""` is the one write a release makes; mirror it into the
          # issue `show` answers from, so a later claim by another actor sees what bd would.
          case "$*" in *"-s open -a"*) printf '%s' "{\"id\":\"$2\",\"status\":\"open\",\"assignee\":\"\",\"labels\":[]}" > "$d/bd.issue.json";; esac
          exit 0;;
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
        .env_remove("AIR_ROLE")
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
        .env_remove("AIR_ROLE")
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

    let (code, out, _) = air(&repo, &bd, &["claim", "zz-1", "--files", "a.rs,b.rs"]);
    assert_eq!(code, 0, "{out}");
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert!(log.contains("update zz-1 --claim --actor tester"), "{log}");
    // bd now holds it in_progress by this actor.
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-1","status":"in_progress","assignee":"tester","labels":[]}"#,
    )
    .unwrap();
    std::fs::write(repo.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(claims(&repo), vec![("zz-1".into(), "main".into(), None)]);

    let (code, _, err) = air(&repo, &bd, &["release", "zz-1", "--reason", "bogus"]);
    assert_eq!(code, 1);
    assert!(err.contains("--reason must be one of"));

    let (code, out, _) = air(&repo, &bd, &["release", "zz-1", "--reason", "abandoned"]);
    assert_eq!(code, 0, "{out}");
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    // air-0kk: one process, status and assignee together (the trailing `-a ""` logs as `-a`).
    assert!(
        log.lines().any(|l| l.trim() == "update zz-1 -s open -a"),
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
        .args(["claim", "zz-9"])
        .env("AIR_BD_BIN", &slow)
        .env("FAKE_BD_DIR", &repo)
        .env("AIR_BD_TIMEOUT_MS", "1000")
        .env("BEADS_ACTOR", "tester")
        .env_remove("AIR_ROLE")
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
        err.contains("bd show zz-9"),
        "must send the reader to bd: {err}"
    );
    assert!(
        !err.contains("refused"),
        "a timeout is not a refusal: {err}"
    );
    // air-gsj: it was retried once, the message says so, and it says what to do.
    assert!(err.contains("retried once"), "{err}");
    assert!(err.contains("re-run `air claim zz-9`"), "{err}");
    // Nothing in the ledger: the row is written only after a confirmed result.
    assert!(claims(&repo).is_empty(), "{:?}", claims(&repo));
    // And the event line carries `timeout`, so `air audit` can count how often it fires.
    let events = std::fs::read_dir(repo.join(".air/events"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    assert!(events.contains(r#""decision":"timeout""#), "{events}");
    assert!(!events.contains(r#""decision":"bd-refused""#), "{events}");
    // air-gsj: exactly one retry between the two timeouts, never a third attempt.
    assert_eq!(
        events.matches(r#""decision":"timeout-retry""#).count(),
        1,
        "{events}"
    );
}

/// air-gsj: bd hangs on the FIRST `--claim` and answers the second. `air claim` retries once
/// internally, the claim lands, and the worker never retried by hand — which is when
/// The adopter's w1 lost to a peer. The retry is recorded as `timeout-retry` and the
/// outcome as `claimed-retried`, so `air audit` counts how often bd's tail bites.
#[test]
fn a_bd_timeout_on_claim_is_retried_once_and_the_retry_lands() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    // The first `update` hangs past the timeout and writes nothing; every later call is the
    // ordinary fake bd, whose `update` succeeds.
    let flaky = repo.join("bd-hang-once");
    std::fs::write(
        &flaky,
        format!(
            "#!/bin/sh\ncase \"$1\" in update) if [ ! -e {mark} ]; then : > {mark}; sleep 3; exit 0; fi; exec {bd} \"$@\";; *) exec {bd} \"$@\";; esac\n",
            mark = repo.join("bd.hung-once").display(),
            bd = bd.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&flaky, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let _ = Command::new(&flaky)
        .arg("show")
        .arg("warm")
        .output()
        .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .args(["claim", "zz-9"])
        .env("AIR_BD_BIN", &flaky)
        .env("FAKE_BD_DIR", &repo)
        .env("AIR_BD_TIMEOUT_MS", "1000")
        .env("BEADS_ACTOR", "tester")
        .env_remove("AIR_ROLE")
        .current_dir(&repo)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("the retry landed"), "{text}");
    // The ledger row is keyed by the worktree (`main` in a scratch repo); `tester` is the
    // bd actor.
    assert_eq!(
        claims(&repo),
        vec![("zz-9".to_string(), "main".to_string(), None)]
    );
    let events = std::fs::read_dir(repo.join(".air/events"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect::<String>();
    assert!(events.contains(r#""decision":"timeout-retry""#), "{events}");
    assert!(
        events.contains(r#""decision":"claimed-retried""#),
        "{events}"
    );
    assert!(!events.contains(r#""decision":"timeout""#), "{events}");
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
            "#!/bin/sh\ncase \"$1\" in update) printf '%s' '{{\"id\":\"zz-9\",\"status\":\"in_progress\",\"assignee\":\"tester\",\"labels\":[],\"updated_at\":\"2020-01-01T00:00:00Z\"}}' > {issue}; sleep 3; exit 0;; *) exec {bd} \"$@\";; esac\n",
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
            .env_remove("AIR_ROLE")
            .current_dir(&repo)
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    let (code, out, err) = run(&["--json", "claim", "zz-9"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(err.contains("confirms the claim landed"), "{err}");
    assert!(!out.contains("nothing was written"), "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let first = v["claimed_at"].as_str().unwrap().to_string();
    assert_eq!(claims(&repo), vec![("zz-9".into(), "main".into(), None)]);

    // A digest written now, after the first claim, declaring the bead it is about (air-agq).
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(repo.join(".claude/air.json"), r#"{"digest_dir":"docs/d"}"#).unwrap();
    std::fs::create_dir_all(repo.join("docs/d")).unwrap();
    std::fs::write(
        repo.join("docs/d/2026-main-zz-9.md"),
        "---\nbead: zz-9\n---\n\ndigest\n",
    )
    .unwrap();

    // Re-claim: bd already holds it by us; no bd write, the row keeps the first time.
    let (code, out, _) = run(&["--json", "claim", "zz-9"]);
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

    let (code, out, _) = air(&repo, &bd, &["claim", "zz-2"]);
    assert_eq!(code, 1);
    assert!(
        out.contains("bd refused the claim; nothing recorded"),
        "{out}"
    );
    assert!(claims(&repo).is_empty());
}

/// air-45pw: through the real argv, because the argv is where this bug lives. `air capture`
/// took one positional and nothing else, so a finding long enough to be worth writing went
/// through the harness's command classifier as a command line and was refused for its shape;
/// an adopter's worker shortened a finding in order to file it.
///
/// The assertion is byte-for-byte against the file. A truncation is what the bug produces, and
/// the capture it produces is still filed, still long, and still reads like a capture — so a
/// test asserting "non-empty" would pass over the exact failure.
#[test]
fn capture_takes_a_whole_file_and_refuses_both_routes_or_neither() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);

    // Far longer than any command line anyone would type, with the quoting and blank lines that
    // make a classifier refuse, and a sentinel last sentence a truncation eats first.
    let mut finding = String::new();
    for i in 0..60 {
        finding.push_str(&format!(
            "Paragraph {i}: the worker's own words, with \"quotes\", a $dollar and a `tick`, \
             running well past what belongs in a shell argument.\n\n"
        ));
    }
    finding.push_str("SENTINEL: the last sentence.");
    let path = repo.join("finding.md");
    std::fs::write(&path, &finding).unwrap();

    let (code, out, err) = air(
        &repo,
        &bd,
        &["--json", "capture", "--file", path.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{out}{err}");

    // Read it back through `air inbox`, which is what the coordinator actually triages from.
    let (_, out, _) = air(&repo, &bd, &["--json", "inbox"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let stored = v["captures"][0]["text"].as_str().unwrap();
    assert_eq!(
        stored, finding,
        "the capture must be the file, byte for byte"
    );
    assert_eq!(stored.len(), finding.len());
    assert!(stored.ends_with("SENTINEL: the last sentence."), "{stored}");

    // Neither route, and both at once: each refusal names both ways in, because the person
    // reading it has just had a capture refused.
    for args in [
        vec!["capture"],
        vec!["capture", "a line", "--file", path.to_str().unwrap()],
    ] {
        let (code, out, err) = air(&repo, &bd, &args);
        let said = format!("{out}{err}");
        assert_eq!(code, 2, "{said}");
        assert!(said.contains("--file"), "{said}");
        assert!(said.contains("text"), "{said}");
    }

    // A missing path is named, never filed as an empty capture.
    let (code, out, err) = air(&repo, &bd, &["capture", "--file", "nope.md"]);
    let said = format!("{out}{err}");
    assert_eq!(code, 2, "{said}");
    assert!(said.contains("nope.md"), "{said}");

    // Still one capture: nothing above filed anything.
    let (_, out, _) = air(&repo, &bd, &["--json", "inbox"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["captures"].as_array().unwrap().len(), 1, "{out}");

    // `--help` names the file route: it is where the person whose capture was just refused
    // looks next, and it named only the positional before this bead.
    let (_, out, _) = air(&repo, &bd, &["capture", "--help"]);
    assert!(out.contains("--file"), "{out}");
}

/// air-6dj4: a capture records where it was written, and `air inbox` shows it.
///
/// Through the real binary, against `git rev-parse HEAD` rather than a value the test built —
/// the acceptance says the stored sha must equal the worktree's head, and a test that compares
/// Air's answer with Air's own answer proves nothing.
///
/// The reported cause was "a capture carries no timestamp", which is false: the row has always
/// carried `captured_at`. The real defect is that the time is on the ROW and the subject is in
/// the BODY, and the body is what gets quoted onward — so "the batch is red" arrives elsewhere
/// with no way to say which batch.
#[test]
fn a_capture_records_the_head_it_was_written_at() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);

    let head = git(&repo, &["rev-parse", "HEAD"]);
    let (code, out, err) = air(&repo, &bd, &["--json", "capture", "the batch is red"]);
    assert_eq!(code, 0, "{out}{err}");

    // The stored value IS git's answer, not a shape that looks like one.
    let (_c, out, err) = air(&repo, &bd, &["--json", "inbox"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["captures"][0]["head"]["At"].as_str(),
        Some(head.as_str()),
        "{out}{err}"
    );

    // And the reader sees it on the row, short.
    let (_c, out, err) = air(&repo, &bd, &["inbox"]);
    assert!(out.contains(&format!("at {}", &head[..8])), "{out}{err}");
    assert!(!out.contains("no head"), "{out}{err}");
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

    let (code, out, _) = air(&repo, &bd, &["triage", &id, "--bead", "zz-7"]);
    assert_eq!(code, 0, "{out}");
    // A resolved capture is re-pointed, not refused (air-76z), and says what it left.
    let (code, out, _) = air(&repo, &bd, &["triage", &id, "--drop", "dup"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("re-pointed from bead zz-7"), "{out}");
    let (_, out, _) = air(&repo, &bd, &["inbox"]);
    assert_eq!(out.trim(), "inbox empty");
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
    let ids: Vec<String> = (1..=10).map(|i| format!("zz-{i}")).collect();
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
    assert!(log.contains("close zz-1 "), "{log}");
    assert!(log.contains("zz-10"), "{log}");
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
    let (code, out, _) = air_env(
        &wt,
        &bd,
        &["close", "zz-1", "--reason", "x"],
        &[("AIR_ROLE", "worker")],
    );
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
    let (code, _, err) = air(&repo, &bd, &["triage", &ids[0], &ids[1], "--bead", "zz-1"]);
    assert_ne!(code, 0, "{err}");
    // Promoted or dropped, never both.
    let (code, _, err) = air(
        &repo,
        &bd,
        &["triage", &ids[0], "--bead", "zz-1", "--drop", "dup"],
    );
    assert_eq!(code, 1, "{err}");

    for (id, args) in [
        (&ids[0], vec!["--bead", "zz-1"]),
        (&ids[1], vec!["--bead", "zz-2"]),
        (&ids[2], vec!["--drop", "dup"]),
    ] {
        let mut argv = vec!["--json", "triage", id.as_str()];
        argv.extend(args);
        let (code, out, err) = air(&repo, &bd, &argv);
        assert_eq!(code, 0, "{out}{err}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["resolved"], 1);
    }
    let (_, out, _) = air(&repo, &bd, &["--json", "triage", &ids[0], "--bead", "zz-9"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["inbox_depth"], 0);
    // A second pass re-points rather than refusing (air-76z).
    assert_eq!(v["repointed"][0]["from"], "bead zz-1");
    assert_eq!(v["repointed"][0]["to"], "bead zz-9");
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
    let (code, out, _) = air(&repo, &bd, &["triage", &id, "--bead", "zz-real"]);
    assert_eq!(code, 0, "{out}");
    let log = std::fs::read_to_string(repo.join("bd.log")).unwrap();
    assert_eq!(
        log.lines().filter(|l| l.starts_with("show ")).count(),
        1,
        "{log}"
    );

    // And the wrong pointer can be corrected after the fact; the event names both ids.
    let (code, out, _) = air(&repo, &bd, &["--json", "triage", &id, "--bead", "zz-fixed"]);
    assert_eq!(code, 0, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["repointed"][0]["from"], "bead zz-real");
    assert_eq!(v["repointed"][0]["to"], "bead zz-fixed");
    let day = std::fs::read_dir(repo.join(".air/events"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .next()
        .unwrap();
    let events = std::fs::read_to_string(&day).unwrap();
    assert!(
        events.contains("re-pointed from bead zz-real to bead zz-fixed"),
        "{events}"
    );

    // A capture id nothing matches is reported, not silently accepted.
    let (code, out, _) = air(
        &repo,
        &bd,
        &["triage", "01NOSUCHCAPTURE", "--bead", "zz-real"],
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
        .args(["triage", &id, "--bead", "zz-1"])
        .env("AIR_BD_BIN", &slow)
        .env("FAKE_BD_DIR", &repo)
        .env("AIR_BD_PROBE_TIMEOUT_MS", "1000")
        .env("BEADS_ACTOR", "tester")
        .env_remove("AIR_ROLE")
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

    says(&wt, r#"{"id":"zz-1","status":"open","labels":["owner"]}"#);
    let (code, out, _) = air_env(&wt, &bd, &["claim", "zz-1"], &[("AIR_ROLE", "worker")]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("is labelled `owner`"), "{out}");
    // air-uef: the refusal no longer sends the worker to an owner inbox that does not exist.
    assert!(out.contains("`air capture` the question"), "{out}");
    assert!(!out.contains("--for owner"), "{out}");

    // `human` is presence, not authority: it does not stop a worker.
    says(&wt, r#"{"id":"zz-2","status":"open","labels":["human"]}"#);
    let (code, out, _) = air(&wt, &bd, &["claim", "zz-2"]);
    assert_eq!(code, 0, "{out}");

    // The coordinator is not gated by it either way.
    says(&repo, r#"{"id":"zz-3","status":"open","labels":["owner"]}"#);
    let (code, out, _) = air(&repo, &bd, &["claim", "zz-3"]);
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

    // air-uef: one queue. `--for owner` is refused naming the replacement; a plain capture
    // lands in the one inbox; and no owner condition exists for the channel to push.
    let (c, o) = run(&wt, &me, &["capture", "--for", "owner", "rule on ports"]);
    assert_eq!(c, 2, "{o}");
    assert!(
        o.contains("air-uef") && o.contains("labelled `owner`"),
        "{o}"
    );
    let (_, o) = run(&repo, &me, &["inbox"]);
    assert!(o.contains("inbox empty"), "{o}");
    let (c, _) = run(&wt, &me, &["capture", "rule on ports"]);
    assert_eq!(c, 0);
    let (_, o) = run(&repo, &me, &["inbox"]);
    assert!(o.contains("rule on ports"), "{o}");
    let (_, o) = run(&repo, &me, &["--json", "status", "--attention"]);
    assert!(!o.contains("owner-decision-waiting"), "{o}");
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
    std::fs::write(repo.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&repo, &bd, &["claim", "zz-1"]).0, 0);

    // The diagnostic, run the documented number of times: still nothing to report.
    for _ in 0..3 {
        let (code, out, err) = air(&repo, &bd, &["handover", "--bead", "zz-1"]);
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
        "tool_input": {"command": "bd close zz-1 --reason done"}}),
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
    assert_eq!(air(&repo, &bd, &["claim", "zz-3"]).0, 0);
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
        "---\nbead: zz-3\n---\n\ndigest\n",
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

/// air-xbl (the adopter, 2026-08-30/31). Two symptoms, one root, both end to end.
///
/// A worktree holding NO claim, in a repo with `digest_dir` configured, used to be refused on
/// `digest-present` unconditionally, with a fix naming a literal `<bead>`: the adopter's
/// batching lane (claims nothing, merges other workers' green shas, verifies once) could not
/// hand over at all. And a worker holding exactly one claim with no digest yet was handed the
/// same placeholder, because the held id was computed for the lookup and thrown away before
/// the message. The back-to-back check the adopter's w3 asked for: `air status` naming the claim
/// and `air handover` naming the same id, in the same state.
#[test]
fn handover_names_the_held_bead_and_skips_the_digest_with_no_claim() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(
        repo.join(".claude/air.json"),
        r#"{"digest_dir":"docs/log.d"}"#,
    )
    .unwrap();
    std::fs::create_dir_all(repo.join("docs/log.d")).unwrap();
    // A digest is even there, declaring some bead; it is not what decides this.
    std::fs::write(
        repo.join("docs/log.d/2026-08-30-main-x.md"),
        "---\nbead: zz-x\n---\n\ndigest\n",
    )
    .unwrap();
    let missing = |o: &str| -> Vec<serde_json::Value> {
        let v: serde_json::Value = serde_json::from_str(o).unwrap();
        v["missing"].as_array().cloned().unwrap_or_default()
    };

    // No claim, no bead named: the digest check is skipped, and nothing prints `<bead>`.
    let (_, o, _) = air(&repo, &bd, &["--json", "handover"]);
    assert!(
        !missing(&o).iter().any(|m| m["check"] == "digest-present"),
        "{o}"
    );
    assert!(!o.contains("<bead>"), "{o}");

    // One claim held, no digest for it: refused, and the refusal names the held id in both
    // the detail and the fix, the same id `air status` prints.
    std::fs::write(repo.join("bd.in_progress"), "zz-251z\n").unwrap();
    assert_eq!(air(&repo, &bd, &["claim", "zz-251z"]).0, 0);
    let (_, st, _) = air(&repo, &bd, &["status"]);
    assert!(st.contains("claims: zz-251z"), "{st}");
    let (code, o, _) = air(&repo, &bd, &["--json", "handover"]);
    let m = missing(&o);
    let d = m.iter().find(|m| m["check"] == "digest-present");
    assert!(d.is_some(), "{o}");
    let d = d.unwrap();
    assert!(
        d["detail"].as_str().unwrap().contains("bead: zz-251z"),
        "{o}"
    );
    assert!(d["fix"].as_str().unwrap().contains("bead: zz-251z"), "{o}");
    assert!(!o.contains("<bead>"), "{o}");
    assert_eq!(code, 0, "advisory outside the hook path: {o}");

    // The digest declaring it satisfies the check; the one for fd-x never did.
    std::fs::write(
        repo.join("docs/log.d/2026-08-30-main-zz-251z.md"),
        "---\nbead: zz-251z\n---\n\ndigest\n",
    )
    .unwrap();
    let (_, o, _) = air(&repo, &bd, &["--json", "handover"]);
    assert!(
        !missing(&o).iter().any(|m| m["check"] == "digest-present"),
        "{o}"
    );
}

/// air-60x (the adopter, 2026-08-31), end to end. Worker A claims fd-1, does the work
/// with a `Bead: fd-1` trailer, and releases it as landed. Worker B then builds a better fix
/// for the same defect on its own branch, carrying fd-1 by trailer and holding no claim on it.
/// `air handover fd-1` from B used to refuse with "not claimed by B, run `air claim fd-1`",
/// while `air land` would have taken the branch on its own criterion. Now the trailer is the
/// path, the refusal (before green) never offers `air claim`, and once green with main merged
/// the hand-over passes. The digest check looks for fd-1 too, because that is the work carried.
#[test]
fn a_superseding_branch_hands_over_by_its_trailer() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    let beta = main.parent().unwrap().join("beta");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-beta",
            beta.to_str().unwrap(),
        ],
    );
    let beta = beta.canonicalize().unwrap();
    std::fs::write(
        main.join(".claude/air.json"),
        r#"{"verify_command": "true", "digest_dir": "docs/log.d"}"#,
    )
    .unwrap();
    git(&main, &["commit", "-q", "-am", "chore: digest dir"]);

    // A: claimed, worked, released as landed. The claim is gone; the bead is A's history.
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    git(
        &alpha,
        &["commit", "-q", "--allow-empty", "-m", &bead_trailer("zz-1")],
    );
    assert_eq!(
        air(&alpha, &bd, &["release", "zz-1", "--reason", "landed"]).0,
        0
    );
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    // B: a better instrument for the same defect, carrying fd-1 by trailer, no claim on it.
    std::fs::write(beta.join("better.txt"), "measured, not rounded\n").unwrap();
    git(&beta, &["add", "better.txt"]);
    git(&beta, &["commit", "-q", "-m", &bead_trailer("zz-1")]);
    let missing = |o: &str| -> Vec<serde_json::Value> {
        let v: serde_json::Value = serde_json::from_str(o).unwrap();
        v["missing"].as_array().cloned().unwrap_or_default()
    };

    // Before merging main and recording a green: refused on those, never on the claim, and
    // nothing offers `air claim fd-1`. The digest check names fd-1, the carried bead.
    let (_, o, _) = air(&beta, &bd, &["--json", "handover", "--bead", "zz-1"]);
    let m = missing(&o);
    assert!(!m.iter().any(|m| m["check"] == "claim"), "{o}");
    assert!(
        m.iter().any(|m| m["check"] == "verify-green-at-head"),
        "{o}"
    );
    assert!(
        m.iter().any(|m| m["check"] == "digest-present"
            && m["detail"].as_str().unwrap().contains("bead: zz-1")),
        "{o}"
    );
    assert!(!o.contains("air claim"), "{o}");
    assert!(!o.contains("<bead>"), "{o}");

    // With no bead named the same branch is checked for the same carried bead.
    let (_, o, _) = air(&beta, &bd, &["--json", "handover"]);
    assert!(
        missing(&o).iter().any(|m| m["check"] == "digest-present"
            && m["detail"].as_str().unwrap().contains("bead: zz-1")),
        "{o}"
    );

    // Digest, main merged, green: handable, by name and unnamed.
    std::fs::create_dir_all(beta.join("docs/log.d")).unwrap();
    std::fs::write(
        beta.join("docs/log.d/2026-08-31-beta-zz-1.md"),
        "---\nbead: zz-1\n---\n\nthe better instrument\n",
    )
    .unwrap();
    git(&beta, &["add", "docs/log.d"]);
    git(&beta, &["commit", "-q", "-m", &bead_trailer("zz-1")]);
    git(&beta, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&beta, &bd, &["record", "verify", "--", "true"]).0, 0);
    let (code, o, e) = air(&beta, &bd, &["--json", "handover", "--bead", "zz-1"]);
    assert_eq!(code, 0, "{o}{e}");
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["pass"], true, "{o}");
    let (_, o, _) = air(&beta, &bd, &["--json", "handover"]);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["pass"], true, "{o}");

    // A bead the branch neither carries nor B claims is still refused, with the trailer as
    // the fix.
    let (code, o, _) = air(&beta, &bd, &["--json", "handover", "--bead", "zz-9"]);
    assert_eq!(code, 0, "advisory: {o}");
    let m = missing(&o);
    assert!(m.iter().any(|m| m["check"] == "claim"), "{o}");
    assert!(o.contains("Bead: zz-9") && !o.contains("air claim"), "{o}");
}

/// air-f10 (the adopter's w2, 2026-08-31), end to end: bd's ready set holds two epics, one
/// owner-labelled bead and one task. The line says one claimable and names the epics apart,
/// and `air claim` on an epic is refused with the reason, so no assignee lands on a container.
#[test]
fn ready_line_names_epics_apart_and_claim_refuses_one() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    std::fs::write(
        repo.join("bd.ready.json"),
        r#"[{"id":"zz-e1","status":"open","issue_type":"epic"},
            {"id":"zz-e2","status":"open","issue_type":"epic"},
            {"id":"zz-own","status":"open","issue_type":"task","labels":["owner"]},
            {"id":"zz-t","status":"open","issue_type":"task"}]"#,
    )
    .unwrap();
    let (code, out, err) = air(&repo, &bd, &["status"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("ready: 4 (1 claimable; 2 epic(s), not claimable; 1 owner-labelled"),
        "{out}"
    );
    // The cache the Stop nudge reads holds only the task.
    let cache = std::fs::read_to_string(repo.join(".air/ready.json")).unwrap();
    assert!(
        cache.contains("zz-t") && !cache.contains("zz-e1"),
        "{cache}"
    );

    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-e1","status":"open","issue_type":"epic","labels":[]}"#,
    )
    .unwrap();
    let (code, out, _) = air(&repo, &bd, &["claim", "zz-e1"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("is an epic"), "{out}");
    assert!(out.contains("claim a child"), "{out}");
}

/// An adopter's fleet protocol (2026-09-25), end to end: `"leases"` in `.claude/air.json` makes Air's PreToolUse hook
/// refuse a declared command from a worker that does not hold the lease — the core of an
/// adopter's own ~1,500-line guard, which read a second lock store. No config, the owner, an
/// unmatched command and a held lease are all allowed; unenforced, the refusal is advice.
#[test]
fn a_declared_command_needs_its_lease() {
    use std::io::Write;
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    // (exit code, stdout + stderr) for one PreToolUse(Bash) from alpha's worktree.
    let hook = |cmd: &str, role: Option<&str>, enforce: bool| -> (i32, String) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_air"));
        c.arg("--repo")
            .arg(&alpha)
            .arg("hook")
            .env("AIR_BD_BIN", &bd)
            .env("FAKE_BD_DIR", &main)
            .env("BEADS_ACTOR", "alpha")
            .env("AIR_ENFORCE", if enforce { "1" } else { "0" })
            .env_remove("AIR_ROLE")
            .current_dir(&alpha)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if let Some(r) = role {
            c.env("AIR_ROLE", r);
        }
        let mut child = c.spawn().unwrap();
        let body = serde_json::json!({
            "session_id": "s-lease", "hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": cmd}, "cwd": alpha.to_string_lossy(),
        });
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (
            out.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        )
    };
    // No `leases` key: nothing needs a lease.
    assert_eq!(hook("make api", Some("worker"), true).0, 0);

    std::fs::write(
        main.join(".claude/air.json"),
        r#"{"verify_command": "true", "leases": {"runtime": ["make api*", "adb *"]}}"#,
    )
    .unwrap();
    let (code, out) = hook("cd app && make api", Some("worker"), true);
    assert_eq!(code, 2, "{out}");
    assert!(
        out.contains("runtime") && out.contains("air lease take runtime"),
        "{out}"
    );
    let (code, out) = hook("adb shell", Some("worker"), false);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("air lease take runtime"), "advisory: {out}");
    assert_eq!(
        hook("make api", None, true).0,
        0,
        "the owner is never asked"
    );
    assert_eq!(hook("make test", Some("worker"), true).0, 0);
    let (_, needs, _) = air(&alpha, &bd, &["lease", "needs", "make api"]);
    assert!(needs.contains("needs runtime"), "{needs}");

    // A live holder: this test process stands in for the session's `claude`.
    let pid = std::process::id().to_string();
    let (code, out, err) = air_env(
        &alpha,
        &bd,
        &["lease", "take", "runtime", "--reason", "api"],
        &[("AIR_LEASE_PID", pid.as_str())],
    );
    assert_eq!(code, 0, "{out}{err}");
    let (code, out) = hook("make api", Some("worker"), true);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("air:"), "held is silent: {out}");
}

/// An adopter's fleet protocol (2026-09-25), end to end: a worker with no claim stopping while a task is ready is nudged
/// to claim it; the same worker named as `verify_lane` in `.claude/air.json` is not, because
/// the lane claims no bead. An adopter recorded the Stop hook offering its lane ready beads.
#[test]
fn stop_nudge_skips_the_verification_lane() {
    use std::io::Write;
    let (_tmp, main, alpha) = land_repo("true");
    // Nothing carried: a worker between beads, which is when the nudge speaks.
    git(&alpha, &["reset", "-q", "--hard", "main"]);
    let bd = fake_bd(&main);
    std::fs::write(
        main.join("bd.ready.json"),
        r#"[{"id":"zz-t","status":"open","issue_type":"task"}]"#,
    )
    .unwrap();
    assert_eq!(air(&main, &bd, &["status"]).0, 0);
    let stop = |session: &str| -> String {
        let mut child = Command::new(env!("CARGO_BIN_EXE_air"))
            .arg("--repo")
            .arg(&main)
            .arg("hook")
            .env("AIR_BD_BIN", &bd)
            .env("FAKE_BD_DIR", &main)
            .env("AIR_ROLE", "worker")
            .env("BEADS_ACTOR", "alpha")
            .current_dir(&alpha)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let body = serde_json::json!({
            "session_id": session, "hook_event_name": "Stop",
            "cwd": alpha.to_string_lossy(), "stop_hook_active": false,
        });
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };
    let worker = stop("s-worker");
    assert!(worker.contains("air claim zz-t"), "{worker}");

    std::fs::write(
        main.join(".claude/air.json"),
        r#"{"verify_command": "true", "verify_lane": "alpha"}"#,
    )
    .unwrap();
    let lane = stop("s-lane");
    assert!(!lane.contains("air claim"), "{lane}");
}

/// air-v7o, end to end. Dirt from a build (an untracked file no tool edited) reads as
/// unjournaled dirt with the report's time; once removed it is no holding. A tool edit the
/// PostToolUse hook journaled reads as an edit with its age, dirty or clean.
#[test]
fn holdings_tags_say_when_and_tell_dirt_from_an_edit() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);

    // Build output: present, uncommitted, never edited by a tool.
    std::fs::write(repo.join("generated.txt"), "artifact\n").unwrap();
    let (code, out, err) = air(&repo, &bd, &["holdings"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("compared 1 worktrees at 20"), "{out}");
    assert!(
        out.contains("generated.txt: main[uncommitted now, no edit journaled]"),
        "{out}"
    );
    std::fs::remove_file(repo.join("generated.txt")).unwrap();
    let (_, out, _) = air(&repo, &bd, &["holdings"]);
    assert!(!out.contains("generated.txt"), "cleaned: {out}");

    // A real edit: the hook journals it, and the tag says so with its age.
    std::fs::write(repo.join("src.rs"), "fn f() {}\n").unwrap();
    let edited = repo.join("src.rs").to_string_lossy().to_string();
    let (code, err) = air_hook(
        &repo,
        &bd,
        serde_json::json!({"hook_event_name": "PostToolUse", "tool_name": "Edit",
        "tool_input": {"file_path": edited}}),
        false,
    );
    assert_eq!(code, 0, "{err}");
    let (_, out, _) = air(&repo, &bd, &["holdings"]);
    assert!(
        out.contains("src.rs: main[uncommitted now, edited 0 min ago]"),
        "{out}"
    );
    // Committed and clean: the edit is remembered with its age, and the tree says clean.
    git(&repo, &["add", "src.rs"]);
    git(&repo, &["commit", "-q", "-m", "src"]);
    let (_, out, _) = air(&repo, &bd, &["holdings"]);
    assert!(
        out.contains("src.rs: main[journaled 0 min ago, clean now]"),
        "{out}"
    );
    let (_, out, _) = air(&repo, &bd, &["--json", "holdings"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["at"].as_str().unwrap().starts_with("20"), "{out}");
    assert_eq!(v["files"]["src.rs"][0]["journaled"], true, "{out}");
    assert!(v["files"]["src.rs"][0]["last_edit"].is_string(), "{out}");
}

/// air-b5k, end to end: a repo whose settings carry the adopter's `SessionStart -> bd prime
/// --hook-json` is told by a dry-run `air install`, with the fix; one without it is told
/// nothing about hooks.
#[test]
fn install_reports_a_stale_bd_prime_hook() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::write(
        repo.join(".claude/settings.json"),
        r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"bd prime --hook-json"}]}]}}"#,
    )
    .unwrap();
    let (code, out, err) = air(&repo, &bd, &["install"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("STALE HOOK: SessionStart runs `bd prime --hook-json`"),
        "{out}"
    );
    assert!(out.contains("do: `bd prime` injects"), "{out}");
    assert!(out.contains("Delete the entry"), "{out}");

    std::fs::write(repo.join(".claude/settings.json"), "{}").unwrap();
    let (code, out, err) = air(&repo, &bd, &["install"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("STALE HOOK"), "{out}");
}

/// air-80x.3, end to end: alpha claims fd-1, commits with the trailer, merges main: batch-ready.
/// A green at its head: gone, reason `green-at-head`. main moves past it: batch-ready again.
#[test]
fn status_lists_batch_ready_branches_as_a_fact() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    git(
        &alpha,
        &["commit", "-q", "--allow-empty", "-m", &bead_trailer("zz-1")],
    );
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    let head = git(&alpha, &["rev-parse", "HEAD"]);
    let read = |out: &str| -> serde_json::Value {
        let v: serde_json::Value = serde_json::from_str(out).unwrap();
        v["snapshot"].clone()
    };

    let (code, out, err) = air(&main, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{out}{err}");
    let s = read(&out);
    assert_eq!(s["batch_ready"][0]["worker"], "alpha", "{out}");
    assert_eq!(s["batch_ready"][0]["head"], head, "{out}");
    assert_eq!(s["batch_ready"][0]["beads"][0], "zz-1", "{out}");
    let (_, text, _) = air(&main, &bd, &["status"]);
    assert!(
        text.contains(&format!("batch-ready: alpha at {} (zz-1)", &head[..8])),
        "{text}"
    );

    // Green at the head: landable on its own, so not for a batch.
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    let (_, out, _) = air(&main, &bd, &["--json", "status"]);
    let s = read(&out);
    assert!(s["batch_ready"].as_array().unwrap().is_empty(), "{out}");
    assert!(
        s["not_batch_ready"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["worker"] == "alpha" && n["check"] == "green-at-head"),
        "{out}"
    );

    // main moves: the green branch is no longer landable, and it is batch-ready again without
    // re-merging, because the lane merges main forward at the cut (an adopter's fleet protocol, 2026-09-25).
    std::fs::write(main.join("README"), "b\n").unwrap();
    git(&main, &["commit", "-q", "-am", "docs: readme"]);
    let (_, out, _) = air(&main, &bd, &["--json", "status"]);
    let s = read(&out);
    assert_eq!(s["batch_ready"][0]["worker"], "alpha", "{out}");
    assert_eq!(s["batch_ready"][0]["head"], head, "{out}");
}

/// Precheck (2026-09-25), end to end: where `.claude/air.json` declares `"precheck": true`, a branch is
/// batch-ready only once `air record precheck` is green at its head, and that green is never a
/// verify green: the branch contains main, so a verify green would make it landable and take it
/// out of the batch (`green-at-head`), and the close gate would stop naming the missing verify.
#[test]
fn a_declared_precheck_gates_batch_ready_and_is_not_a_verify() {
    let (_tmp, main, alpha) = land_repo("true");
    std::fs::write(
        main.join(".claude/air.json"),
        r#"{"verify_command": "true", "precheck": true}"#,
    )
    .unwrap();
    let bd = fake_bd(&main);
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    git(
        &alpha,
        &["commit", "-q", "--allow-empty", "-m", &bead_trailer("zz-1")],
    );
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    let head = git(&alpha, &["rev-parse", "HEAD"]);
    let status = || -> serde_json::Value {
        let (code, out, err) = air(&main, &bd, &["--json", "status"]);
        assert_eq!(code, 0, "{out}{err}");
        serde_json::from_str::<serde_json::Value>(&out).unwrap()["snapshot"].clone()
    };

    let s = status();
    assert!(s["batch_ready"].as_array().unwrap().is_empty(), "{s}");
    let not = s["not_batch_ready"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["worker"] == "alpha")
        .cloned()
        .unwrap();
    assert_eq!(not["check"], "no-precheck", "{s}");
    assert!(
        not["detail"]
            .as_str()
            .unwrap()
            .contains("air record precheck --"),
        "{s}"
    );

    // A red precheck is not enough; a green one at this head is.
    assert_eq!(
        air(&alpha, &bd, &["record", "precheck", "--", "false"]).0,
        1
    );
    assert!(status()["batch_ready"].as_array().unwrap().is_empty());
    let (code, o, e) = air(&alpha, &bd, &["record", "precheck", "--", "true"]);
    assert_eq!(code, 0, "{o}{e}");
    assert!(o.contains("recorded green precheck for alpha"), "{o}");
    let s = status();
    assert_eq!(s["batch_ready"][0]["worker"], "alpha", "{s}");
    assert_eq!(s["batch_ready"][0]["head"], head, "{s}");

    // Not a verify: the close gate still wants one.
    let (_, o, _) = air(&alpha, &bd, &["--json", "handover"]);
    let v: serde_json::Value = serde_json::from_str(&o).unwrap();
    assert!(
        v["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["check"] == "verify-green-at-head"),
        "{o}"
    );

    // A new commit is a new head: the precheck at the old one does not carry over.
    git(
        &alpha,
        &["commit", "-q", "--allow-empty", "-m", &bead_trailer("zz-1")],
    );
    let s = status();
    assert!(s["batch_ready"].as_array().unwrap().is_empty(), "{s}");
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
    std::fs::write(repo.join("bd.in_progress"), "zz-1\n").unwrap();
    let (code, _, _) = air(&repo, &bd, &["claim", "zz-1"]);
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
        .env_remove("AIR_ROLE")
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
    assert_eq!(main["claims"][0]["bead"], "zz-1", "claim kept: {o}");
    assert!(v["duration_ms"].as_u64().unwrap() < 3000, "{o}");
}

/// air-6p5: a green branch waited on a merge and nothing said so. `air status` lists the
/// landable branches with their exact commands, derived from git plus the ledger rather than
/// stored twice. (The owner inbox that also listed them went with air-uef; landing has been
/// the coordinator's since air-3pz, so `air status` is the one surface.)
///
/// air-7kp: the fixture is a worker BRANCH whose commit names the bead. A coordinator's own
/// checkout is never a landing candidate — there is nothing to merge into main from main.
#[test]
fn status_lists_green_landings_with_their_commands() {
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
    std::fs::write(repo.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    std::fs::write(repo.join("bd.in_progress"), "").unwrap();
    std::fs::write(alpha.join("work.txt"), "w\n").unwrap();
    g(&alpha, &["add", "work.txt"]);
    g(&alpha, &["commit", "-q", "-m", &bead_trailer("zz-1")]);
    let head = g(&alpha, &["rev-parse", "HEAD"]);

    // Not green at that head yet: the worker's to fix, so nothing is landable.
    let landable = |out: &str| -> Vec<serde_json::Value> {
        let v: serde_json::Value = serde_json::from_str(out).unwrap();
        v["snapshot"]["landable"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|l| l["bead"] == "zz-1" && l["blocked"].is_null())
            .collect()
    };
    let (code, out, err) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(landable(&out).is_empty(), "{out}");

    let (code, o, e) = air(&alpha, &bd, &["record", "verify", "--", "true"]);
    assert_eq!(code, 0, "{o}{e}");
    let (code, out, err) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{out}{err}");
    let l = landable(&out);
    assert_eq!(l.len(), 1, "{out}");
    assert_eq!(l[0]["worker"], "alpha", "{out}");
    assert_eq!(l[0]["head"], head, "{out}");
    assert_eq!(l[0]["command"], "air land --worker alpha", "{out}");
}

/// air-6u5: the adopter's case end to end, with NO trailer anywhere.
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

    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    // Prose only: this is a repo with no `Bead:` trailers, which is every repo that has not
    // adopted them yet.
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt"]);
    git(&alpha, &["commit", "-q", "-m", "feat: the work (zz-1)"]);

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
    assert_eq!(v["landed"][0], "zz-1", "{out}");
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
    // air-155w: the fix names a command that is safe under both flows. `air record verify`
    // is the one a verify lane forbids that worker, and this used to require it.
    assert!(out.contains("air handover"), "the fixing command: {out}");
    assert!(!out.contains("air record verify"), "{out}");

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

/// air-0kk: worker A claims and releases; worker B claims. Before the fix B was refused by
/// Air's own rule (a pencilled assignee blocks every other `--claim` in bd 1.2.x), because
/// the release reopened the bead and left A pencilled in. The adopter's and this repo's
/// air-an9 both sat in `bd ready` in that state.
#[test]
fn a_released_bead_is_claimable_by_another_worker() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    let bd = fake_bd(&repo);
    let as_actor = |actor: &str, args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_air"))
            .arg("--repo")
            .arg(&repo)
            .args(args)
            .env("AIR_BD_BIN", &bd)
            .env("FAKE_BD_DIR", &repo)
            .env("BEADS_ACTOR", actor)
            .env_remove("AIR_ROLE")
            .current_dir(&repo)
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        )
    };
    let (code, text) = as_actor("alpha", &["claim", "zz-1"]);
    assert_eq!(code, 0, "{text}");
    // bd holds it in_progress by alpha (the stub's `update --claim` writes no state).
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-1","status":"in_progress","assignee":"alpha","labels":[]}"#,
    )
    .unwrap();
    let (code, text) = as_actor("alpha", &["release", "zz-1", "--reason", "reassigned"]);
    assert_eq!(code, 0, "{text}");
    // The stub mirrored the one write a release makes: open, assignee cleared.
    let issue = std::fs::read_to_string(repo.join("bd.issue.json")).unwrap();
    assert!(issue.contains(r#""assignee":"""#), "{issue}");
    // Another actor can claim it now. This was refused before air-0kk.
    let (code, text) = as_actor("beta", &["claim", "zz-1"]);
    assert_eq!(code, 0, "{text}");
    assert!(!text.contains("pencilled assignee"), "{text}");
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
            .env_remove("AIR_ROLE")
            .env("AIR_TMUX_SOCKET", &socket)
            .current_dir(&repo)
            .output()
            .unwrap()
    };

    tmux(&["new-session", "-d", "-s", "zz-main", "sleep", "30"]);
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-1","title":"the window says what the lane is doing","status":"open","labels":[]}"#,
    )
    .unwrap();
    assert_eq!(air_tmux(&["claim", "zz-1"]).status.code(), Some(0));
    let labelled = window();

    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-1","title":"t","status":"in_progress","assignee":"tester","labels":[]}"#,
    )
    .unwrap();
    let out = air_tmux(&["release", "zz-1", "--reason", "abandoned"]);
    let cleared = window();
    tmux(&["kill-server"]);

    assert_eq!(out.status.code(), Some(0));
    assert!(labelled.starts_with("zz-1 the window says"), "{labelled}");
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

    std::fs::write(repo.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&repo, &bd, &["claim", "zz-1"]).0, 0);
    let claimed_at = row("zz-1").0;

    // The stray flip: bd holds nothing in_progress and shows the bead in awaiting_review.
    std::fs::write(repo.join("bd.in_progress"), "").unwrap();
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-1","status":"awaiting_review","labels":[]}"#,
    )
    .unwrap();
    let (code, o, _) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{o}");
    let main = worker_view(&o);
    assert_eq!(main["handed_over"][0]["bead"], "zz-1", "still held: {o}");
    assert!(main["claims"].as_array().unwrap().is_empty(), "{o}");
    let (at, handover, released, _) = row("zz-1");
    assert_eq!(at, claimed_at, "original claim time kept");
    assert!(released.is_none(), "the claim must stay open");
    assert!(handover.is_some(), "marked handed over");

    // Reverted: bd holds it in_progress again, and the row is as it was.
    std::fs::write(repo.join("bd.in_progress"), "zz-1\n").unwrap();
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-1","status":"in_progress","assignee":"tester","labels":[]}"#,
    )
    .unwrap();
    let (code, o, _) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{o}");
    let main = worker_view(&o);
    assert_eq!(main["claims"][0]["bead"], "zz-1", "claimed again: {o}");
    assert_eq!(
        row("zz-1"),
        (claimed_at.clone(), handover, None, None),
        "row untouched by the revert"
    );

    // Closed: the claim is released, with `closed` as the reason.
    std::fs::write(repo.join("bd.in_progress"), "").unwrap();
    std::fs::write(
        repo.join("bd.issue.json"),
        r#"{"id":"zz-1","status":"closed","labels":[]}"#,
    )
    .unwrap();
    let (code, o, _) = air(&repo, &bd, &["--json", "status"]);
    assert_eq!(code, 0, "{o}");
    let (at, _, released, reason) = row("zz-1");
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
/// air-80x.1: a verify lane's green at a batch commit closes the bead it covers, and a batch
/// cut before the worker's last commit does not. End to end: alpha commits with a `Bead:`
/// trailer; a lane worktree merges main and alpha's branch and records the only green, at the
/// batch head, as worker `lane`; alpha's `air handover --bead fd-1` passes on it with no green
/// at alpha's HEAD. Then alpha commits more for fd-1, and the same batch is refused by name.
#[test]
fn a_lane_batch_green_closes_the_bead_it_covers_and_a_stale_batch_is_named() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    let dead = &[("AIR_ATTRIBUTION_FALLBACK_BEFORE", "2000-01-01T00:00:00Z")];
    // alpha's commit carries the bead by trailer (land_repo's first commit does not).
    std::fs::write(alpha.join("more.txt"), "more\n").unwrap();
    git(&alpha, &["add", "-A"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: zz-1\n"],
    );
    // The lane: a worktree off main that merges alpha and verifies once.
    let lane = main.parent().unwrap().join("lane");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-lane",
            lane.to_str().unwrap(),
        ],
    );
    // `--no-ff`: a batch is a merge commit of its own, never alpha's head renamed. A
    // fast-forward here would put the green AT alpha's HEAD and exercise nothing new.
    git(
        &lane,
        &[
            "merge",
            "-q",
            "--no-ff",
            "worktree-alpha",
            "-m",
            "batch: alpha",
        ],
    );
    assert_eq!(
        air_env(&lane, &bd, &["record", "verify", "--", "true"], dead).0,
        0
    );
    // No green at alpha's HEAD; the batch's green covers fd-1's one commit.
    let (code, out, err) = air_env(&alpha, &bd, &["--json", "handover", "--bead", "zz-1"], dead);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["pass"], true, "{out}");
    assert!(
        v["message"].as_str().unwrap().contains("batch by lane"),
        "{out}"
    );
    assert!(
        v["message"]
            .as_str()
            .unwrap()
            .contains("every commit of zz-1"),
        "{out}"
    );

    // alpha commits again for fd-1 after the batch was cut: the batch no longer covers it.
    std::fs::write(alpha.join("late.txt"), "late\n").unwrap();
    git(&alpha, &["add", "-A"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: after the cut\n\nBead: zz-1\n"],
    );
    let late = git(&alpha, &["rev-parse", "--short=8", "HEAD"]);
    let (_, out, _) = air_env(&alpha, &bd, &["--json", "handover", "--bead", "zz-1"], dead);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["pass"], false, "{out}");
    let msg = v["message"].as_str().unwrap();
    assert!(msg.contains("predates your commit"), "{msg}");
    assert!(
        msg.contains(&late),
        "must name the commit after the cut: {msg}"
    );
    assert!(msg.contains("after the cut"), "{msg}");
    assert!(msg.contains("next batch"), "{msg}");
}

/// air-80x.2: a verify lane's batch lands as ONE landing. Three worker branches, each carrying
/// its bead by trailer and NO green of its own; a lane worktree merges main and all three
/// (`--no-ff`) and records the only green at the batch head; `air land --worker lane` from the
/// main checkout lands once, attributes every bead once, records the three member heads on the
/// row, and leaves every worker's head an ancestor of main so their next `git merge main` is
/// a fast-forward.
#[test]
fn a_lane_batch_lands_once_with_every_bead_and_its_members_recorded() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    let dead = &[("AIR_ATTRIBUTION_FALLBACK_BEFORE", "2000-01-01T00:00:00Z")];
    let root = main.parent().unwrap().to_path_buf();
    // alpha already exists from land_repo; add beta and gamma the same way.
    let mut workers: Vec<(String, PathBuf)> = vec![("alpha".into(), alpha.clone())];
    for name in ["beta", "gamma"] {
        let wt = root.join(name);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &format!("worktree-{name}"),
                wt.to_str().unwrap(),
            ],
        );
        workers.push((name.to_string(), wt.canonicalize().unwrap()));
    }
    let beads = ["zz-1", "zz-2", "zz-3"];
    let mut heads: Vec<String> = Vec::new();
    for ((name, wt), bead) in workers.iter().zip(beads) {
        std::fs::write(main.join("bd.in_progress"), format!("{bead}\n")).unwrap();
        assert_eq!(air_env(wt, &bd, &["claim", bead], dead).0, 0, "{name}");
        std::fs::write(wt.join(format!("{name}.txt")), format!("{name}\n")).unwrap();
        git(wt, &["add", &format!("{name}.txt")]);
        git(wt, &["commit", "-q", "-m", &bead_trailer(bead)]);
        heads.push(git(wt, &["rev-parse", "HEAD"]));
    }
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    // The lane: main plus every branch, one merge commit each, one green at the end.
    let lane = root.join("lane");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-lane",
            lane.to_str().unwrap(),
        ],
    );
    for (name, _) in &workers {
        git(
            &lane,
            &[
                "merge",
                "-q",
                "--no-ff",
                &format!("worktree-{name}"),
                "-m",
                &format!("batch: {name}"),
            ],
        );
    }
    assert_eq!(
        air_env(&lane, &bd, &["record", "verify", "--", "true"], dead).0,
        0
    );
    acceptance(&main, "- Verify recorded green at HEAD.\n");
    let before = git(&main, &["rev-parse", "HEAD"]);

    let (code, out, err) = air_env(&main, &bd, &["land", "--worker", "lane"], dead);
    assert_eq!(code, 0, "{out}{err}");
    for bead in beads {
        assert!(out.contains(bead), "{out}");
    }
    assert_ne!(git(&main, &["rev-parse", "HEAD"]), before);

    // One landing row, every bead once, every member head recorded.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let rows: Vec<(String, String, String, String)> = conn
        .prepare("SELECT worker, result, beads, members FROM landings")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let (worker, result, beads_json, members_json) = &rows[0];
    assert_eq!((worker.as_str(), result.as_str()), ("lane", "landed"));
    let landed: Vec<String> = serde_json::from_str(beads_json).unwrap();
    let mut sorted = landed.clone();
    sorted.sort();
    assert_eq!(sorted, beads, "every bead once: {landed:?}");
    let members: Vec<serde_json::Value> = serde_json::from_str(members_json).unwrap();
    let mut member_pairs: Vec<(String, String)> = members
        .iter()
        .map(|m| {
            (
                m["worker"].as_str().unwrap().to_string(),
                m["sha"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    member_pairs.sort();
    let mut expected: Vec<(String, String)> = workers
        .iter()
        .map(|(n, _)| n.clone())
        .zip(heads.iter().cloned())
        .collect();
    expected.sort();
    assert_eq!(
        member_pairs, expected,
        "the batch's members are the three worker heads"
    );
    // Every worker's head is now in main: their next `git merge main` is a fast-forward.
    for (name, _) in &workers {
        assert_eq!(
            git(
                &main,
                &[
                    "merge-base",
                    "--is-ancestor",
                    &format!("worktree-{name}"),
                    "main"
                ]
            ),
            "",
            "{name}'s head is not an ancestor of main"
        );
    }
}

/// `air batch cut` (2026-09-25), end to end: `air batch cut` in the lane's worktree. Four batch-ready
/// members: alpha clean; beta and gamma both rewrite `shared.txt`, gamma ready first by its
/// commit time though beta sorts first by name; delta rewrites `m.txt`, which main rewrote after
/// delta branched. The dry run names both drops and changes nothing; the cut leaves the lane's
/// head holding main plus exactly alpha's and gamma's commits, and each drop is an event line.
/// The main checkout is refused.
#[test]
fn batch_cut_drops_by_the_order_rule_and_merges_the_rest() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    let root = main.parent().unwrap().to_path_buf();
    // Air's own merges need an identity; worktrees share the main checkout's config.
    git(&main, &["config", "user.name", "air"]);
    git(&main, &["config", "user.email", "air@example.invalid"]);
    for f in ["shared.txt", "m.txt"] {
        std::fs::write(main.join(f), "base\n").unwrap();
    }
    git(&main, &["add", "shared.txt", "m.txt"]);
    git(&main, &["commit", "-q", "-m", "base files"]);
    let mut wts = vec![("alpha", alpha.clone())];
    for name in ["beta", "gamma", "delta", "lane"] {
        let wt = root.join(name);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &format!("worktree-{name}"),
                wt.to_str().unwrap(),
            ],
        );
        if name != "lane" {
            wts.push((name, wt.canonicalize().unwrap()));
        }
    }
    let lane = root.join("lane").canonicalize().unwrap();
    // (worker, bead, file, content, committer date): the date is the order rule's input.
    let work = [
        (
            "alpha",
            "zz-1",
            "alpha.txt",
            "alpha\n",
            "2026-09-03T00:00:00Z",
        ),
        (
            "beta",
            "zz-2",
            "shared.txt",
            "beta\n",
            "2026-09-02T00:00:00Z",
        ),
        (
            "gamma",
            "zz-3",
            "shared.txt",
            "gamma\n",
            "2026-09-01T00:00:00Z",
        ),
        ("delta", "zz-4", "m.txt", "delta\n", "2026-09-04T00:00:00Z"),
    ];
    let mut heads = std::collections::BTreeMap::new();
    for ((name, wt), (w, bead, file, body, date)) in wts.iter().zip(work) {
        assert_eq!(*name, w);
        std::fs::write(main.join("bd.in_progress"), format!("{bead}\n")).unwrap();
        assert_eq!(air(wt, &bd, &["claim", bead]).0, 0, "{name}");
        std::fs::write(wt.join(file), body).unwrap();
        git(wt, &["add", file]);
        let out = Command::new("git")
            .arg("-C")
            .arg(wt)
            .args(["commit", "-q", "-m", &bead_trailer(bead)])
            .env("GIT_AUTHOR_NAME", "air")
            .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
            .env("GIT_COMMITTER_NAME", "air")
            .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .unwrap();
        assert!(out.status.success(), "{name}: {out:?}");
        heads.insert(w, git(wt, &["rev-parse", "HEAD"]));
    }
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    std::fs::write(main.join("m.txt"), "main moved\n").unwrap();
    git(&main, &["commit", "-q", "-am", "main rewrites m.txt"]);
    let main_tip = git(&main, &["rev-parse", "HEAD"]);

    // The main checkout is refused, naming where to run it.
    let (code, out, err) = air(&main, &bd, &["batch", "cut"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(out.contains("refused in the main checkout"), "{out}");

    let lane_before = git(&lane, &["rev-parse", "HEAD"]);
    let check_drops = |v: &serde_json::Value| {
        let d = v["dropped"].as_array().unwrap();
        assert_eq!(d.len(), 2, "{v}");
        assert!(
            d.iter().any(|d| d["worker"] == "beta"
                && d["against"] == "gamma"
                && d["against_sha"] == heads["gamma"].as_str()
                && d["paths"] == serde_json::json!(["shared.txt"])),
            "{v}"
        );
        assert!(
            d.iter().any(|d| d["worker"] == "delta"
                && d["against"] == "main"
                && d["paths"] == serde_json::json!(["m.txt"])),
            "{v}"
        );
        let kept: Vec<&str> = v["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["worker"].as_str().unwrap())
            .collect();
        assert_eq!(kept, ["gamma", "alpha"], "oldest ready first: {v}");
    };

    let (code, out, err) = air(&lane, &bd, &["--json", "batch", "cut", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    check_drops(&serde_json::from_str(&out).unwrap());
    assert_eq!(
        git(&lane, &["rev-parse", "HEAD"]),
        lane_before,
        "dry run moved"
    );

    let (code, out, err) = air(&lane, &bd, &["--json", "batch", "cut"]);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    check_drops(&v);
    let head = git(&lane, &["rev-parse", "HEAD"]);
    assert_eq!(v["head"], head.as_str(), "{v}");
    assert!(
        v["next"]
            .as_str()
            .unwrap()
            .starts_with("air record verify -- ")
    );
    assert!(git(&lane, &["ls-files", "-u"]).is_empty());
    // Exactly main plus alpha's and gamma's commits.
    let is_anc = |sha: &str| {
        Command::new("git")
            .arg("-C")
            .arg(&lane)
            .args(["merge-base", "--is-ancestor", sha, "HEAD"])
            .status()
            .unwrap()
            .success()
    };
    assert!(is_anc(&main_tip) && is_anc(&heads["alpha"]) && is_anc(&heads["gamma"]));
    assert!(!is_anc(&heads["beta"]) && !is_anc(&heads["delta"]));
    let listed = |range: &str| -> std::collections::BTreeSet<String> {
        git(&lane, &["rev-list", "--no-merges", range])
            .lines()
            .map(str::to_string)
            .collect()
    };
    let mut want = listed(&format!("main..{}", heads["alpha"]));
    want.extend(listed(&format!("main..{}", heads["gamma"])));
    assert_eq!(listed("main..HEAD"), want);

    // Each drop is an event line naming the member and the other side.
    let events: String = std::fs::read_dir(main.join(".air/events"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap())
        .collect();
    let drops: Vec<&str> = events
        .lines()
        .filter(|l| {
            l.contains("\"command\":\"batch-cut\"") && l.contains("\"decision\":\"dropped\"")
        })
        .collect();
    // Two from the dry run's pre-check, two from the cut.
    assert_eq!(drops.len(), 4, "{events}");
    assert!(
        drops
            .iter()
            .any(|l| l.contains("beta") && l.contains("conflicts with gamma"))
    );
}

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
    // A file that exists at the landed commit and that the branch does NOT touch, so an
    // acceptance clause naming it is refuted rather than unresolvable (air-dqa).
    std::fs::create_dir_all(main.join("docs")).unwrap();
    std::fs::write(main.join("docs/rule.md"), "the rule\n").unwrap();
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
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air_env(&alpha, &bd, &["claim", "zz-1"], dead).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    std::fs::create_dir_all(alpha.join("docs/d")).unwrap();
    std::fs::write(
        alpha.join("docs/d/2026-08-29-alpha-zz-1.md"),
        "---\nbead: zz-1\n---\n\ndigest\n",
    )
    .unwrap();
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt", "docs/d"]);
    git(&alpha, &["commit", "-q", "-m", &bead_trailer("zz-1")]);
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
    assert_eq!(v["landed"][0], "zz-1", "{out}");
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

/// air-bxe, narrowed by air-odv: the landings row exists from before main moves, not from the
/// exit, so a killed land leaves a row saying so.
///
/// The adopter's coordinator reported a land done three times before the process exited, then
/// fell back to `pgrep`, which misled them twice. Separately a land killed by a closed pipe
/// (`air land | head`) merged, verified and wrote nothing, leaving main green at a sha no
/// landing mentioned.
///
/// air-odv removed the long window: main is fast-forwarded onto an already-green commit, so
/// there is no minutes-long verify with a rollback armed. The window that produced their
/// incident was the acceptance fetch from bd (~1.4 s per bead) AFTER the fast-forward and
/// before the outcome was recorded; air-bh4 moved that fetch BEFORE the merge, so bd not
/// answering refuses with main untouched and the post-merge window holds no bd call at all.
///
/// The bd stub still snapshots `.air/` when it is asked for acceptance, and that snapshot is
/// now the proof of air-bh4's ordering: at the moment bd is consulted, no landings row exists
/// and nothing has moved. The in-flight write still precedes `merge --ff-only` in
/// `land_one` (air-bxe), which the row's `tip_sha` and `merge_commit` show afterwards.
#[test]
fn a_landing_is_recorded_before_main_moves_and_survives_a_kill() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    std::fs::write(main.join("bd.peek"), "").unwrap();

    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: zz-1\n"],
    );
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    acceptance(&main, "- Verify recorded green at HEAD.\n");

    let before = git(&main, &["rev-parse", "HEAD"]);
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 0, "{out}{err}");
    let head = git(&main, &["rev-parse", "HEAD"]);
    assert_ne!(head, before);

    // What the ledger said when bd was asked for acceptance: nothing yet, because that read
    // now happens before any row is written and before main moves (air-bh4).
    let seen = rusqlite::Connection::open(main.join("seen_air/ledger.db")).unwrap();
    let mid: i64 = seen
        .query_row("SELECT count(*) FROM landings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        mid, 0,
        "acceptance is read before the landing row exists and before main moves"
    );

    // Afterwards ONE row carries the outcome, naming the commit main was moved onto, the sha
    // it was at before, and the process that did it: one attempt, not two.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let (merge, tip, pid): (String, String, i64) = conn
        .query_row("SELECT merge_commit, tip_sha, pid FROM landings", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(merge, head, "it names the commit main was moved onto");
    assert_eq!(tip, before, "with the sha main was at before");
    assert!(pid > 0, "and the process to ask about, so nobody greps");
    let rows: Vec<(String, i64)> = conn
        .prepare("SELECT result, attempt_no FROM landings")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows, vec![("landed".to_string(), 1)]);
    // air-odv: and no verify ran here. The evidence is the worker's own green.
    let land_verifies: i64 = conn
        .query_row(
            "SELECT count(*) FROM verify_runs WHERE trigger='land'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(land_verifies, 0, "landing re-verifies nothing");
}

/// air-gazh: a bead closed with proof whose commits are in no tree but its author's worktree.
///
/// Through the real path, because the join is the thing that did not exist — both halves were
/// already computed and nothing put them together. The probe pins the ledger half and the
/// rendering; this pins that `select` and the closed set actually meet.
///
/// The setup IS the reported incident, minus the lane: two green landable branches, one lands,
/// which moves main and leaves the other behind it through nobody's error. Then that worker's
/// bead closes. Nothing is wrong locally — bead closed, branch green, tree clean — and the
/// commits exist in exactly one worktree.
#[test]
fn a_closed_bead_on_a_branch_behind_main_is_named_and_an_open_one_is_not() {
    let (_tmp, main, alpha) = land_repo("true");
    let root = main.parent().unwrap().to_path_buf();
    let beta = root.join("beta");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-beta",
            beta.to_str().unwrap(),
        ],
    );
    let bd = fake_bd(&main);

    for (wt, bead, file) in [(&alpha, "zz-1", "a.txt"), (&beta, "zz-2", "b.txt")] {
        std::fs::write(main.join("bd.in_progress"), format!("{bead}\n")).unwrap();
        assert_eq!(air(wt, &bd, &["claim", bead]).0, 0);
        std::fs::write(wt.join(file), "work\n").unwrap();
        git(wt, &["add", file]);
        git(
            wt,
            &[
                "commit",
                "-q",
                "-m",
                &format!("feat: work\n\nBead: {bead}\n"),
            ],
        );
        git(wt, &["merge", "-q", "main", "-m", "merge main"]);
        assert_eq!(air(wt, &bd, &["record", "verify", "--", "true"]).0, 0);
    }

    let stranded = |out: &str| -> Vec<(String, String)> {
        let v: serde_json::Value = serde_json::from_str(out).unwrap();
        v["snapshot"]["closed_not_landed"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|c| {
                (
                    c["bead"].as_str().unwrap_or_default().to_string(),
                    c["worker"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    };

    // Land zz-1. Main moves, so beta is now behind it — no error anywhere.
    std::fs::write(main.join("bd.in_progress"), "zz-2\n").unwrap();
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 0, "{out}{err}");

    // zz-2 is OPEN on that unlanded branch, which is the ordinary state of work in flight and
    // must say nothing. Pinned through the real path, not asserted of the join alone: this is
    // the half that reaches the join and has to be rejected there.
    let (_c, out, err) = air(&main, &bd, &["--json", "status"]);
    assert!(stranded(&out).is_empty(), "{out}{err}");

    // Now zz-2 closes. bd no longer holds it in progress and reports it closed, which is what
    // the reconcile reads; nothing about beta's tree or branch changes.
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    std::fs::write(
        main.join("bd.issue.json"),
        r#"[{"id":"zz-2","status":"closed","labels":[]}]"#,
    )
    .unwrap();

    let (_c, out, err) = air(&main, &bd, &["--json", "status"]);
    assert_eq!(
        stranded(&out),
        vec![("zz-2".to_string(), "beta".to_string())],
        "{out}{err}"
    );
    // zz-1 is closed AND landed, and produces nothing: its branch is in main, so it never
    // reaches the join at all.
    assert!(
        !stranded(&out).iter().any(|(b, _)| b == "zz-1"),
        "{out}{err}"
    );

    // It is a CONDITION, not only a printed line (air-gazh): the reporting incident is a
    // coordinator with the fleet view who was not looking.
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let fired: Vec<String> = v["attention"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|a| a["kind"] == "closed-not-landed")
        .map(|a| a["detail"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(fired.len(), 1, "{out}{err}");
    let detail = fired.first().map(String::as_str).unwrap_or_default();
    assert!(
        detail.contains("zz-2") && detail.contains("beta"),
        "{detail}"
    );

    // And the printed surface says it too, for a reader arriving after the push went quiet.
    let (_c, out, err) = air(&main, &bd, &["status"]);
    assert!(
        out.contains("closed, not landed: beta (zz-2)"),
        "{out}{err}"
    );
}

/// air-y3v: after a land, the branches left behind read as needing a re-merge, and `air land`
/// refuses them with the same reason the list gave.
///
/// The incident (owner, 2026-08-29, third time in one hour): the owner inbox (gone since
/// air-uef) listed a branch as landable and printed `air land <bead>` beside it; `air land`
/// then refused the same
/// branch for not containing main. Two surfaces, one fact, different answers — the landable list
/// checked only for a recorded green at the head. Every land invalidates the containment
/// condition for every other branch, so the list went stale the instant a land succeeded.
///
/// This drives the real sequence: two green branches, land one, then ask both surfaces about
/// the other and try to land it.
#[test]
fn after_a_land_the_other_branch_reads_as_needing_a_remerge() {
    let (_tmp, main, alpha) = land_repo("true");
    let root = main.parent().unwrap().to_path_buf();
    let beta = root.join("beta");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-beta",
            beta.to_str().unwrap(),
        ],
    );
    let bd = fake_bd(&main);

    // Two branches, both green at a head that contains main, both landable.
    for (wt, bead, file) in [(&alpha, "zz-1", "a.txt"), (&beta, "zz-2", "b.txt")] {
        std::fs::write(main.join("bd.in_progress"), format!("{bead}\n")).unwrap();
        assert_eq!(air(wt, &bd, &["claim", bead]).0, 0);
        std::fs::write(wt.join(file), "work\n").unwrap();
        git(wt, &["add", file]);
        git(
            wt,
            &[
                "commit",
                "-q",
                "-m",
                &format!("feat: work\n\nBead: {bead}\n"),
            ],
        );
        git(wt, &["merge", "-q", "main", "-m", "merge main"]);
        assert_eq!(air(wt, &bd, &["record", "verify", "--", "true"]).0, 0);
    }
    std::fs::write(main.join("bd.in_progress"), "").unwrap();

    // The command `air status` offers for each branch (air-uef: the owner inbox that also
    // listed them is gone; `air status` is the one surface).
    let offered = |out: &str| -> std::collections::BTreeMap<String, String> {
        let v: serde_json::Value = serde_json::from_str(out).unwrap();
        v["snapshot"]["landable"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|l| {
                (
                    l["bead"].as_str().unwrap().to_string(),
                    l["command"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    // Both offered with `air land` while both are actually landable.
    let (_c, out, err) = air(&main, &bd, &["--json", "status"]);
    let cmds = offered(&out);
    assert_eq!(
        cmds.get("zz-1").map(String::as_str),
        Some("air land --worker alpha"),
        "{out}{err}"
    );
    assert_eq!(
        cmds.get("zz-2").map(String::as_str),
        Some("air land --worker beta"),
        "{out}{err}"
    );

    // air-5wq: beta's ok line names the main it was true of.
    let main_before = git(&main, &["rev-parse", "HEAD"]);
    let (code, out, err) = air(&beta, &bd, &["handover"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(&format!("containing main {}", &main_before[..7])),
        "{out}{err}"
    );

    // Land one. This moves main past beta's branch point.
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 0, "{out}{err}");

    // air-4up: beta's own gate names the external cause — the landing that moved main, from
    // whom, and where main is now — instead of describing beta's tree. Same fix.
    let landed = git(&main, &["rev-parse", "HEAD"]);
    let (_c, out, err) = air(&beta, &bd, &["handover"]);
    assert!(
        out.contains(&landed[..7]) && out.contains("(landing from alpha)"),
        "{out}{err}"
    );
    assert!(out.contains("main is not an ancestor of HEAD"), "{out}");
    // air-155w: merging is required under both flows and stays a command; recording a green
    // is the clause a lane forbids and is now a condition.
    assert!(out.contains("git merge main"), "{out}");
    assert!(!out.contains("air record verify"), "{out}");
    // air-5wq: and names the main it compared against, which is not the one the ok line
    // named, so the pair reads as main having moved rather than as a contradiction.
    assert!(
        out.contains(&format!("main is at {}", &landed[..7])) && landed != main_before,
        "{out}"
    );

    // The incident: beta is still shown, because work IS waiting...
    let (_c, out, err) = air(&main, &bd, &["--json", "status"]);
    let cmds = offered(&out);
    let fd2 = cmds.get("zz-2").cloned().unwrap_or_default();
    assert!(cmds.contains_key("zz-2"), "still shown: {out}{err}");
    // ...but never with the command that cannot work.
    assert_ne!(
        fd2, "air land --worker beta",
        "must not offer a land it would refuse: {out}"
    );
    assert!(fd2.contains("git merge main"), "{out}");

    // `air status` says the same thing.
    let (_c, st, se) = air(&main, &bd, &["status"]);
    assert!(st.contains("waiting, not landable"), "{st}{se}");
    assert!(st.contains("zz-2") && st.contains("git merge main"), "{st}");
    // And does not announce it as landable.
    let (_c, att, _e) = air(&main, &bd, &["status", "--attention"]);
    assert!(!att.contains("landable"), "{att}");

    // And the command refuses with the same reason the list gave.
    let (code, out, err) = air(&main, &bd, &["land", "zz-2"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(out.contains("does not contain main"), "{out}{err}");
    assert!(out.contains("git merge main"), "{out}");
    // `--all` too: nothing landable, and it says which branch and what to do.
    let (code, out, _e) = air(&main, &bd, &["land", "--all"]);
    assert_eq!(code, 2, "{out}");
    assert!(
        out.contains("beta") && out.contains("git merge main"),
        "{out}"
    );

    // Re-merge and re-verify, and it is landable again by both surfaces.
    git(&beta, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&beta, &bd, &["record", "verify", "--", "true"]).0, 0);
    let (_c, out, _e) = air(&main, &bd, &["--json", "status"]);
    assert_eq!(
        offered(&out).get("zz-2").map(String::as_str),
        Some("air land --worker beta"),
        "{out}"
    );
    let (code, out, err) = air(&main, &bd, &["land", "zz-2"]);
    assert_eq!(code, 0, "{out}{err}");
}

/// air-09b: the adopter's two observed cases, end to end. A batching lane (beta) merges alpha's
/// branch, so both ranges name fd-1. Naming the bead is refused with both carriers and the
/// `--worker` command for each: never the oldest-waiting branch, which is what landed the
/// wrong one on 2026-08-30. When alpha's green goes stale the bead is still refused, now with
/// beta's command and alpha's fix, rather than either refused outright or silently landing
/// beta. `--worker beta` lands beta with every bead its range names, recorded as such.
#[test]
fn a_bead_on_two_branches_is_refused_and_worker_names_the_branch() {
    let (_tmp, main, alpha) = land_repo("true");
    let root = main.parent().unwrap().to_path_buf();
    let beta = root.join("beta");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-beta",
            beta.to_str().unwrap(),
        ],
    );
    let bd = fake_bd(&main);

    // alpha does fd-1 and hands it on (the claim is released, as close-with-proof does).
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    std::fs::write(alpha.join("a.txt"), "work\n").unwrap();
    git(&alpha, &["add", "a.txt"]);
    git(&alpha, &["commit", "-q", "-m", "feat: a\n\nBead: zz-1\n"]);
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    assert_eq!(
        air(&alpha, &bd, &["release", "zz-1", "--reason", "landed"]).0,
        0
    );
    // beta is the batching lane: it takes alpha's branch and adds fd-2 on top.
    std::fs::write(main.join("bd.in_progress"), "zz-1\nzz-2\n").unwrap();
    assert_eq!(air(&beta, &bd, &["claim", "zz-1"]).0, 0);
    assert_eq!(air(&beta, &bd, &["claim", "zz-2"]).0, 0);
    git(
        &beta,
        &["merge", "-q", "worktree-alpha", "-m", "batch alpha"],
    );
    std::fs::write(beta.join("b.txt"), "work\n").unwrap();
    git(&beta, &["add", "b.txt"]);
    git(&beta, &["commit", "-q", "-m", "feat: b\n\nBead: zz-2\n"]);
    git(&beta, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&beta, &bd, &["record", "verify", "--", "true"]).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    let before = git(&main, &["rev-parse", "HEAD"]);

    // Case 1: both landable. The bead is refused, both carriers named, main untouched.
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        out.contains("air land --worker alpha") && out.contains("air land --worker beta"),
        "{out}{err}"
    );
    assert_eq!(git(&main, &["rev-parse", "HEAD"]), before, "nothing landed");
    // The surface offers the branch form, one entry per (branch, bead). (`air status` is the
    // one surface since the owner inbox went with air-uef.)
    let (_c, st, _e) = air(&main, &bd, &["--json", "status"]);
    assert!(
        st.contains("air land --worker alpha") && st.contains("air land --worker beta"),
        "{st}"
    );

    // Case 2: main moves (a docs commit) and only beta re-merges. alpha is still green at its
    // head but behind main: blocked, with a fix. The bead is still refused, still both named:
    // beta with its command, alpha with its fix. Not refused outright, not landed by state.
    // (A branch whose green is merely stale is SKIPPED before its beads are read, so Air
    // cannot see it as a carrier at all; this is the blocked case the adopter observed.)
    std::fs::write(main.join("README"), "b\n").unwrap();
    git(&main, &["commit", "-q", "-am", "docs: readme"]);
    let before = git(&main, &["rev-parse", "HEAD"]);
    git(&beta, &["merge", "-q", "main", "-m", "merge main"]);
    assert_eq!(air(&beta, &bd, &["record", "verify", "--", "true"]).0, 0);
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(out.contains("air land --worker beta"), "{out}{err}");
    assert!(
        out.contains("alpha") && out.contains("does not contain main"),
        "alpha's own reason and fix: {out}"
    );
    assert!(!out.contains("air land --worker alpha"), "{out}");
    assert_eq!(git(&main, &["rev-parse", "HEAD"]), before, "nothing landed");

    // The selector: beta lands, carrying both beads, and the row records both.
    let (code, out, err) = air(&main, &bd, &["land", "--worker", "beta"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("landed beta") && out.contains("zz-1") && out.contains("zz-2"),
        "{out}{err}"
    );
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let beads: String = conn
        .query_row("SELECT beads FROM landings", [], |r| r.get(0))
        .unwrap();
    assert!(beads.contains("zz-1") && beads.contains("zz-2"), "{beads}");
    // Everything alpha had went in with beta, so no listed branch carries fd-1 now: the
    // missing-bead refusal, unchanged. (A bead on ONE blocked branch is the refusal
    // `after_a_land_the_other_branch_reads_as_needing_a_remerge` drives.)
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(out.contains("no green branch names zz-1"), "{out}{err}");
    // `--worker` on a branch the selection does not list is a refusal that names the branch,
    // never a silent no-op: alpha is already in main, and there is no `nobody`.
    for w in ["alpha", "nobody"] {
        let (code, out, err) = air(&main, &bd, &["land", "--worker", w]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(out.contains(&format!("worktree-{w}")), "{out}{err}");
    }
}

/// air-75u: the adopter end to end. The coordinator's shell sits in alpha's worktree
/// when its turn ends, so the Stop hook's `cwd` is alpha's. With the launcher's `AIR_ROLE`
/// the session is still main: no hand-over check, and the session row is main's. Without it
/// (a session Air did not launch) the checkout decides, and the advisory at least says whose
/// tree it is about.
#[test]
fn a_coordinator_whose_shell_is_in_a_worktree_is_still_the_coordinator() {
    use std::io::Write;
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    // alpha holds a claim and is not green, so a worker's Stop would speak.
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    std::fs::write(alpha.join("more.txt"), "x\n").unwrap();
    git(&alpha, &["add", "more.txt"]);
    git(&alpha, &["commit", "-q", "-m", "wip"]);

    let stop = |session: &str, env: &[(&str, &str)]| -> String {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_air"));
        cmd.arg("--repo")
            .arg(&main)
            .arg("hook")
            .env("AIR_BD_BIN", &bd)
            .env("FAKE_BD_DIR", &main)
            .env_remove("AIR_ROLE")
            .env_remove("BEADS_ACTOR")
            .current_dir(&main)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().unwrap();
        let body = serde_json::json!({
            "session_id": session, "hook_event_name": "Stop",
            "cwd": alpha.to_string_lossy(), "stop_hook_active": false,
        });
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };
    let stop_events = || -> Vec<serde_json::Value> {
        let mut lines = Vec::new();
        for e in std::fs::read_dir(main.join(".air/events")).unwrap() {
            let text = std::fs::read_to_string(e.unwrap().path()).unwrap();
            lines.extend(
                text.lines()
                    .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
                    .filter(|v| v["command"] == "hook.Stop"),
            );
        }
        lines
    };

    // Launched as the coordinator: no hand-over check, whatever the shell's directory, and
    // the session row is main's.
    let out = stop("coord", &[("AIR_ROLE", "coordinator")]);
    assert!(!out.contains("handover"), "{out}");
    let ev = stop_events();
    let last = ev.last().unwrap();
    assert_eq!(last["worker"], "main", "{last}");
    assert!(
        last["reason"]
            .as_str()
            .unwrap()
            .contains("coordinator: no hand-over check"),
        "{last}"
    );
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let who: String = conn
        .query_row(
            "SELECT worker FROM sessions WHERE session_id='coord'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(who, "main");

    // Launched as a worker: the advisory says whose tree it is.
    let out = stop("w", &[("AIR_ROLE", "worker"), ("BEADS_ACTOR", "alpha")]);
    assert!(out.contains("handover would refuse for alpha at"), "{out}");
    let ev = stop_events();
    assert_eq!(
        ev.last().unwrap()["worker"],
        "alpha",
        "{}",
        ev.last().unwrap()
    );

    // Not launched by Air: the owner, whatever directory the shell is in, so no hand-over
    // advice. The directory never decides a role (owner ruling, 2026-09-14).
    let out = stop("bare", &[]);
    assert!(!out.contains("handover"), "{out}");
}

/// air-ob0, narrowed by air-odv: a rewound merge that a worktree still carries is still named.
///
/// The adopter, 2026-08-23: *"A rollback un-lands a branch from main but cannot un-merge it from
/// anyone who took it."* A worker who merged main during the armed window keeps the rewound
/// commits: a recorded green for a tree main will never have.
///
/// **air-odv removed the way to create one.** Main is fast-forwarded onto an already-green
/// commit, so no landing can rewind, and no NEW rewound row can be written. What survives is
/// the reading: two rewound rows exist in this repo's ledger from before today, and a reader
/// who finds one needs to know who still carries it. So this drives the report from a seeded
/// row rather than from a red land, because a red land is no longer reachable.
///
/// Removal condition for the whole mechanism: when no `rewound` landing row is still carried by
/// any worktree, there is nothing left for it to say.
#[test]
fn a_carried_rewound_merge_is_still_named_from_history() {
    let (_tmp, main, alpha) = land_repo("true");
    let root = main.parent().unwrap().to_path_buf();
    let beta = root.join("beta");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-beta",
            beta.to_str().unwrap(),
        ],
    );
    let bd = fake_bd(&main);
    // Seed the ledger, which `air status` needs anyway.
    assert_eq!(air(&main, &bd, &["status"]).0, 0);

    // A commit beta carries and main does not: exactly the shape a rewind used to leave.
    let tip = git(&main, &["rev-parse", "HEAD"]);
    std::fs::write(beta.join("stranded.txt"), "unverified\n").unwrap();
    git(&beta, &["add", "stranded.txt"]);
    git(&beta, &["commit", "-q", "-m", "the un-landed merge"]);
    let stranded = git(&beta, &["rev-parse", "HEAD"]);
    // ...and beta keeps working on top of it, so the report has to answer CONTAINMENT rather
    // than equality. That distinction is what the first version of this test missed.
    std::fs::write(beta.join("more.txt"), "more\n").unwrap();
    git(&beta, &["add", "more.txt"]);
    git(&beta, &["commit", "-q", "-m", "beta keeps working"]);

    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    conn.execute(
        "INSERT INTO landings (id, worker, sha, tip_sha, result, attempt_no, beads, \
         merge_commit, started_at, finished_at) \
         VALUES ('hist','alpha',?1,?2,'rewound',1,'[\"zz-1\"]',?1,'t0','t1')",
        rusqlite::params![stranded, tip],
    )
    .unwrap();

    let carried = |repo: &Path| -> String {
        air(repo, &bd, &["status"])
            .1
            .lines()
            .filter(|l| l.starts_with("rewound and still carried"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    // The carrier LIST, not just the word: alpha never took it, and the main checkout is where
    // the rewind returned to. Both would be wrong answers and both are one substring away.
    let line = carried(&main);
    assert!(line.contains("already in beta:"), "{line}");
    let _ = &alpha;

    // Self-clearing, with no expiry to choose: nobody carries it, nothing is reported.
    git(&beta, &["reset", "--hard", &tip]);
    assert!(carried(&main).is_empty(), "{}", carried(&main));
}

/// air-3pz: the coordinator merges a green hand-over, verifies the *merged* result, closes the
/// bead in one bd process, releases the claim, and records the landing.
#[test]
fn land_merges_verifies_closes_and_records() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    close_with_proof(&main, &alpha, &bd, "zz-1");
    // air-ayp: acceptance Air can point at evidence for — a green at the landed sha, and a
    // file the merge changed. Anything else would land merged-but-not-closed.
    acceptance(
        &main,
        "- Verify recorded green at HEAD.\n- docs/note.md carries the note.\n",
    );
    let before = git(&main, &["rev-parse", "HEAD"]);

    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("landed alpha (zz-1)"), "{out}{err}");

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
    assert!(out.contains("zz-1 — every clause discharged"), "{out}{err}");
    let log = std::fs::read_to_string(main.join("bd.log")).unwrap();
    assert!(!log.contains("close zz-1"), "{log}");
    // And the landing is a row.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let (worker, result, merge, verify): (String, String, String, Option<String>) = conn
        .query_row(
            "SELECT worker, result, merge_commit, verify_run_id FROM landings",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!((worker.as_str(), result.as_str()), ("alpha", "landed"));
    assert_eq!(merge, head);
    // air-odv: no verify run decided it, because none ran. The evidence is the worker's own
    // green at the branch head, and main's tree is byte-identical to the tree that green
    // describes — which is why the second verify was removable rather than merely expensive.
    assert_eq!(verify, None, "no verify run to point at");
    let land_verifies: i64 = conn
        .query_row(
            "SELECT count(*) FROM verify_runs WHERE trigger='land'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(land_verifies, 0);
    assert_eq!(
        git(&main, &["rev-parse", "HEAD^{tree}"]),
        git(&main, &["rev-parse", "worktree-alpha^{tree}"]),
        "main carries exactly the tree alpha recorded a green for"
    );
}

/// air-ayp: `air land` closes nothing — the worker closes its own bead with proof (owner
/// ruling, 2026-08-22). The landing PRINTS every bead beside its acceptance and Air's verdict
/// per clause, which is the only external check on that honour system. A clause the merge
/// A clause naming a file the merge did not change is kept on the `landings` row and
/// named by `air status` — as a lookup that did not answer, never as a contradiction
/// (air-k6uh: six of nine such firings were clauses that held).
#[test]
fn land_prints_acceptance_closes_nothing_and_flags_a_refuted_clause() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    close_with_proof(&main, &alpha, &bd, "zz-1");
    // Four clauses: one Air can look up, one it can look up and refute (a file that exists and
    // the merge left alone), one it cannot read, and one naming a path-like token that is no
    // file at the landed commit — the adopter's possessive (air-dqa) — which must read as
    // unresolvable, never as a contradiction.
    acceptance(
        &main,
        "- Verify recorded green at HEAD.\n\
         - docs/rule.md says the rule.\n\
         - The owner rules on the counter-argument.\n\
         - The `docs/note.md`'s section is updated.\n",
    );
    let before = git(&main, &["rev-parse", "HEAD"]);

    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 0, "{out}{err}");
    // It MERGED: the code is in main, so nothing is held hostage to the prose.
    assert_ne!(git(&main, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(main.join("work.txt")).unwrap(),
        "the work\n"
    );
    // The print: every clause with its verdict, so a wrong close is visible as it lands.
    assert!(out.contains("air land closes nothing"), "{out}");
    // air-rud0: the tick carries the lookup that produced it, so it cannot be read as a
    // stronger claim than "a lookup matched".
    assert!(
        out.contains("ok (a green verify is recorded at the landed sha) — Verify recorded green"),
        "{out}"
    );
    assert!(!out.contains("ok   Verify recorded"), "bare tick: {out}");
    assert!(out.contains("MISS docs/rule.md says the rule."), "{out}");
    assert!(
        out.contains("?    The owner rules on the counter-argument."),
        "{out}"
    );
    // air-dqa: read end to end through `git ls-tree` at the landed commit. The merge changed
    // docs/note.md; the token is `docs/note.md`'s; Air says it cannot resolve it, and says
    // nothing about the merge not changing it.
    assert!(
        out.contains("?    The `docs/note.md`'s section is updated.")
            && out.contains("cannot resolve it"),
        "{out}"
    );
    assert!(!out.contains("did not change docs/note.md"), "{out}");
    // air-k6uh: the headline reports the lookup, and says in as many words that it is
    // not a contradiction, because six of nine were clauses that held.
    assert!(out.contains("zz-1 — UNCONFIRMED"), "{out}");
    assert!(
        !out.contains("REFUTED"),
        "no contradiction is claimed: {out}"
    );
    // It closes NOTHING, and writes no bd status of any kind.
    let log = std::fs::read_to_string(main.join("bd.log")).unwrap();
    assert!(!log.contains("close zz-1"), "{log}");
    assert!(
        !log.lines().any(|l| l.starts_with("update zz-1 -s")),
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
        open.contains("zz-1") && open.contains("docs/rule.md"),
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
        s.contains("zz-1 landed in")
            && s.contains("naming a file this merge did not change")
            && s.contains("NOT a contradiction")
            && !s.contains("CONTRADICTS"),
        "{s}"
    );
    // The claim really is reconciled away by now, so this is not passing by accident.
    let released: i64 = rusqlite::Connection::open(main.join(".air/ledger.db"))
        .unwrap()
        .query_row(
            "SELECT count(*) FROM claims WHERE bead='zz-1' AND released_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(released, 1, "the claim is gone and the report survives it");

    // Somebody reopens the bead: that is what dealing with it looks like, and it clears.
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    let (_, s, _) = air(&main, &bd, &["status"]);
    assert!(!s.contains("zz-1 landed in"), "cleared once reopened: {s}");
}

/// air-odv: **a branch without a green cannot move main at all**, and a land that fails at any
/// step leaves main exactly where it was. A dirty main no longer refuses.
///
/// This replaces air-3pz's rewind test, whose subject no longer exists. That test asserted a
/// red verify on the merged result put main back with `git reset --hard`. There is no merged
/// result to verify and nothing to put back: main is fast-forwarded onto a commit that is
/// already green, so main is never at an unverified sha and no rewind can occur.
///
/// The dirty-main refusal went with it. Its only reason was that `git reset --hard` would have
/// eaten uncommitted work; `merge --ff-only` declines by itself if a local change is actually
/// in the way, and leaves unrelated ones alone. That refusal blocked three lands on 2026-08-29.
#[test]
fn a_branch_without_a_green_cannot_move_main_and_a_dirty_main_no_longer_refuses() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    std::fs::write(main.join("bd.in_progress"), "zz-1\n").unwrap();
    assert_eq!(air(&alpha, &bd, &["claim", "zz-1"]).0, 0);
    std::fs::write(alpha.join("done.txt"), "done\n").unwrap();
    git(&alpha, &["add", "done.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: zz-1\n"],
    );
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    let before = git(&main, &["rev-parse", "HEAD"]);
    let branch_head = git(&main, &["rev-parse", "worktree-alpha"]);

    // No recorded green: main must not move, and nothing may be merged, built, or rewound.
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 2, "{out}{err}");
    assert_eq!(git(&main, &["rev-parse", "HEAD"]), before, "main untouched");

    // A red verify is not a green: same answer, and still no movement.
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "false"]).0, 1);
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 2, "{out}{err}");
    assert_eq!(git(&main, &["rev-parse", "HEAD"]), before, "main untouched");
    let log = std::fs::read_to_string(main.join("bd.log")).unwrap_or_default();
    assert!(!log.contains("close zz-1"), "nothing closed: {log}");

    // No landing row either, and that is right rather than a gap: since air-7kp the selection
    // reads git and the ledger, so a branch with no green never reaches `land_one` at all.
    let conn = rusqlite::Connection::open(main.join(".air/ledger.db")).unwrap();
    let landings: i64 = conn
        .query_row("SELECT count(*) FROM landings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(landings, 0, "nothing was attempted, so nothing is recorded");

    // Green, and main dirty in a file the landing does not touch: it lands anyway.
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    std::fs::write(main.join("README"), "edited by the owner\n").unwrap();
    let (code, out, err) = air(&main, &bd, &["land", "zz-1"]);
    assert_eq!(code, 0, "a dirty main is no longer a refusal: {out}{err}");
    assert_ne!(git(&main, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(main.join("README")).unwrap(),
        "edited by the owner\n",
        "and the owner's uncommitted change survives"
    );
    assert_eq!(
        git(&main, &["rev-parse", "worktree-alpha"]),
        branch_head,
        "the branch is untouched"
    );

    // New work on the branch, then main moves on underneath it: the branch no longer contains
    // main, and the recorded green is not a green of what would land.
    git(&main, &["checkout", "-q", "--", "README"]);
    std::fs::write(alpha.join("more.txt"), "more\n").unwrap();
    git(&alpha, &["add", "more.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: more\n\nBead: zz-2\n"],
    );
    assert_eq!(air(&alpha, &bd, &["record", "verify", "--", "true"]).0, 0);
    git(&main, &["commit", "-q", "--allow-empty", "-m", "b"]);
    let (code, out, err) = air(&main, &bd, &["land", "zz-2"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        out.contains("does not contain main")
            && out.contains("git merge main")
            && !out.contains("air record verify"),
        "{out}"
    );

    // Across every attempt in this test — two with no green, one landing, one stale branch —
    // nothing ever rewound, because nothing can.
    let rewounds: i64 = conn
        .query_row(
            "SELECT count(*) FROM landings WHERE result='rewound'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rewounds, 0);
}

/// air-3pz: a worker may not land. The coordinator's deny list keeps `git commit` off main,
/// so this is the one allowed path onto it and it must not be a worker's.
#[test]
fn land_refuses_a_worker_and_an_unlandable_bead() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    close_with_proof(&main, &alpha, &bd, "zz-1");

    let (code, out, _) = air_env(&alpha, &bd, &["land", "zz-1"], &[("AIR_ROLE", "worker")]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("air handover"), "{out}");

    let (code, out, _) = air(&main, &bd, &["land", "zz-9"]);
    assert_eq!(code, 2, "{out}");
    // air-7kp: what is landable comes from the merge range now, so the refusal points at
    // `air status` rather than the owner queue.
    assert!(out.contains("no green branch names zz-9"), "{out}");
    assert!(out.contains("air status"), "{out}");
}

/// air-9ij, end to end, and the limb the adopter measured: **a batch green goes on covering
/// its bead after main moves.** alpha commits for fd-1, a lane batches and records the only
/// green, then main gains an ordinary prose commit — no landing, nothing to do with alpha —
/// and alpha merges it. The close used to be refused with no mention of the batch at all,
/// because `contains main` was asked of main as it stood at the moment of the question. The
/// window was never closed by the worker; it was closed by whoever last wrote to main.
///
/// The same test carries the limb that did NOT bite, because it was reported as if it had:
/// starting the next bead costs nothing. The gate is content-based on the bead's own trailer
/// commits and never compares the batch with HEAD, so a commit for fd-2 leaves fd-1 closable.
#[test]
fn a_batch_green_still_covers_its_bead_after_main_moves_and_after_the_next_bead_starts() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    let dead = &[("AIR_ATTRIBUTION_FALLBACK_BEFORE", "2000-01-01T00:00:00Z")];
    std::fs::write(alpha.join("more.txt"), "more\n").unwrap();
    git(&alpha, &["add", "more.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: fd-1\n"],
    );
    let lane = main.parent().unwrap().join("lane");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-lane",
            lane.to_str().unwrap(),
        ],
    );
    git(
        &lane,
        &[
            "merge",
            "-q",
            "--no-ff",
            "worktree-alpha",
            "-m",
            "batch: alpha",
        ],
    );
    assert_eq!(
        air_env(&lane, &bd, &["record", "verify", "--", "true"], dead).0,
        0
    );
    let ask = || -> serde_json::Value {
        let (_, out, _) = air_env(&alpha, &bd, &["--json", "handover", "--bead", "fd-1"], dead);
        serde_json::from_str(&out).unwrap()
    };
    let batch = git(&lane, &["rev-parse", "--short=8", "HEAD"]);
    let v = ask();
    assert_eq!(v["pass"], true, "{v}");

    // The next bead starts on the same branch. fd-1's own commits are unchanged, so the batch
    // still covers it; this limb was reported as a separate failure and is not one.
    std::fs::write(alpha.join("next.txt"), "next\n").unwrap();
    git(&alpha, &["add", "next.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the next bead\n\nBead: fd-2\n"],
    );
    let v = ask();
    assert_eq!(v["pass"], true, "next bead must not close the window: {v}");

    // Main moves under everyone: an ordinary commit by the coordinator, no landing involved.
    std::fs::write(main.join("PROSE.md"), "coordinator prose\n").unwrap();
    git(&main, &["add", "PROSE.md"]);
    git(&main, &["commit", "-q", "-m", "docs: prose"]);
    // Before merging it, the refusal is about main not being an ancestor and NOTHING else:
    // the batch green is still the batch green.
    let v = ask();
    let msg = v["message"].as_str().unwrap().to_string();
    assert_eq!(v["pass"], false, "{msg}");
    assert!(msg.contains("main-merged"), "{msg}");
    assert!(!msg.contains("verify-green-at-head"), "{msg}");

    // Merge it, as the roles flow says to, and the close passes on the batch cut before it.
    git(&alpha, &["merge", "-q", "main", "-m", "merge main"]);
    let v = ask();
    let msg = v["message"].as_str().unwrap().to_string();
    assert_eq!(v["pass"], true, "{msg}");
    assert!(
        msg.contains(&batch),
        "must name the batch it passed on: {msg}"
    );
    assert!(msg.contains("every commit of fd-1"), "{msg}");
}

/// air-9ij, limb 1: **a bead whose every commit is already in main closes on the landing that
/// put it there.** The lane's batch lands, so the worker's next `git merge main` is a
/// fast-forward and `main..HEAD` is empty; the gate read that as "this bead has no commits"
/// and refused, in this repo and in every adopter that keys green by commit. The landing had
/// already required a green at a head containing main, so the proof was never missing.
///
/// A repo keyed by tree never saw this: the landing commit's tree is the batch's tree, so a
/// tree-keyed green stood at HEAD anyway. `land_repo` keys by commit, which is why it bites
/// here.
#[test]
fn a_bead_already_in_main_closes_on_its_landing() {
    let (_tmp, main, alpha) = land_repo("true");
    let bd = fake_bd(&main);
    let dead = &[("AIR_ATTRIBUTION_FALLBACK_BEFORE", "2000-01-01T00:00:00Z")];
    std::fs::write(main.join("bd.in_progress"), "fd-1\n").unwrap();
    assert_eq!(air_env(&alpha, &bd, &["claim", "fd-1"], dead).0, 0);
    std::fs::write(main.join("bd.in_progress"), "").unwrap();
    std::fs::write(alpha.join("more.txt"), "more\n").unwrap();
    git(&alpha, &["add", "more.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: the work\n\nBead: fd-1\n"],
    );
    let lane = main.parent().unwrap().join("lane");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-lane",
            lane.to_str().unwrap(),
        ],
    );
    git(
        &lane,
        &[
            "merge",
            "-q",
            "--no-ff",
            "worktree-alpha",
            "-m",
            "batch: alpha",
        ],
    );
    assert_eq!(
        air_env(&lane, &bd, &["record", "verify", "--", "true"], dead).0,
        0
    );
    acceptance(&main, "- Verify recorded green at HEAD.\n");
    let (code, out, err) = air_env(&main, &bd, &["land", "--worker", "lane"], dead);
    assert_eq!(code, 0, "{out}{err}");

    git(&alpha, &["merge", "-q", "main"]);
    assert_eq!(
        git(&alpha, &["log", "--format=%H", "main..HEAD"]),
        "",
        "the landing leaves the worker nothing main does not have"
    );
    let (_, out, _) = air_env(&alpha, &bd, &["--json", "handover", "--bead", "fd-1"], dead);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let msg = v["message"].as_str().unwrap().to_string();
    assert_eq!(v["pass"], true, "{msg}");
    assert!(msg.contains("already in main"), "{msg}");
    assert!(msg.contains("required a green containing main"), "{msg}");

    // A bead nothing landed is still refused: the landing row is what proves the work is
    // there, so an empty range on its own closes nothing.
    let (_, out, _) = air_env(&alpha, &bd, &["--json", "handover", "--bead", "fd-9"], dead);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["pass"], false, "{out}");
}

/// air-vsvt, end to end: a batch's recorded members are the shas the batch TOOK, not where the
/// branches happen to be when the lane gets round to recording.
///
/// The lane merges alpha and beta, then alpha commits again before the lane's `air record`
/// finishes — a window of minutes in a real round, since the lane's verify is the slowest thing
/// in it. Alpha's head stops being an ancestor of the batch, and the old shape dropped alpha
/// from the list entirely and wrote that to the row, where it was wrong for good. An adopter's
/// lane saw it five times in one night.
///
/// The recorded member for alpha must be the sha the lane merged, never alpha's later head.
#[test]
fn a_batchs_recorded_members_are_the_shas_it_took_not_where_the_branches_moved_to() {
    let (_tmp, main, alpha) = land_repo("false"); // a RED batch: that is what gets reported
    let bd = fake_bd(&main);
    let dead = &[("AIR_ATTRIBUTION_FALLBACK_BEFORE", "2000-01-01T00:00:00Z")];
    let root = main.parent().unwrap().to_path_buf();

    std::fs::write(alpha.join("a.txt"), "a\n").unwrap();
    git(&alpha, &["add", "a.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: alpha\n\nBead: fd-1\n"],
    );
    let alpha_taken = git(&alpha, &["rev-parse", "HEAD"]);

    let beta = root.join("beta");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-beta",
            beta.to_str().unwrap(),
        ],
    );
    let beta = beta.canonicalize().unwrap();
    std::fs::write(beta.join("b.txt"), "b\n").unwrap();
    git(&beta, &["add", "b.txt"]);
    git(&beta, &["commit", "-q", "-m", "feat: beta\n\nBead: fd-2\n"]);
    let beta_taken = git(&beta, &["rev-parse", "HEAD"]);

    // A worker the lane does NOT merge: it must not appear, then or now.
    let gamma = root.join("gamma");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-gamma",
            gamma.to_str().unwrap(),
        ],
    );
    let gamma = gamma.canonicalize().unwrap();
    std::fs::write(gamma.join("g.txt"), "g\n").unwrap();
    git(&gamma, &["add", "g.txt"]);
    git(
        &gamma,
        &["commit", "-q", "-m", "feat: gamma\n\nBead: fd-3\n"],
    );

    let lane = root.join("lane");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-lane",
            lane.to_str().unwrap(),
        ],
    );
    for w in ["alpha", "beta"] {
        git(
            &lane,
            &[
                "merge",
                "-q",
                "--no-ff",
                &format!("worktree-{w}"),
                "-m",
                &format!("batch: {w}"),
            ],
        );
    }

    // THE WINDOW: alpha commits after the lane merged it and before the lane records.
    std::fs::write(alpha.join("late.txt"), "late\n").unwrap();
    git(&alpha, &["add", "late.txt"]);
    git(
        &alpha,
        &["commit", "-q", "-m", "feat: after the cut\n\nBead: fd-1\n"],
    );
    let alpha_now = git(&alpha, &["rev-parse", "HEAD"]);
    assert_ne!(alpha_now, alpha_taken, "the window has to be real");

    air_env(&lane, &bd, &["record", "verify", "--", "false"], dead);

    let conn = rusqlite::Connection::open(main.join(".air").join("ledger.db")).unwrap();
    let members: String = conn
        .query_row(
            "SELECT members FROM verify_runs WHERE worker='lane' ORDER BY finished_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&members).unwrap();
    let by = |w: &str| -> Option<String> {
        v.as_array()?.iter().find(|m| m["worker"] == w)?["sha"]
            .as_str()
            .map(str::to_string)
    };
    // alpha is a member, at the sha the batch took, NOT where it has since moved to.
    assert_eq!(by("alpha"), Some(alpha_taken), "members: {members}");
    assert_ne!(by("alpha"), Some(alpha_now), "members: {members}");
    assert_eq!(by("beta"), Some(beta_taken), "members: {members}");
    // A branch the batch never took is not invented into it.
    assert_eq!(by("gamma"), None, "members: {members}");

    // And the line the lane reads names both members it actually carried.
    let (_, out, _) = air_env(&main, &bd, &["--json", "status"], dead);
    assert!(out.contains("alpha"), "{out}");
    assert!(out.contains("beta"), "{out}");
}

/// air-kexg, end to end: a branch whose only commits are session-journal entries lands with no
/// bead, and one that mixes them with anything else still needs a trailer.
///
/// Two workers concluded independently that a journal branch could land — the journal is per
/// session, ungated, and explicitly not work on a bead, so a journal commit is the one commit a
/// worker legitimately writes that names none. It could not, and the refusal sent them to amend
/// a commit that is not about a bead.
///
/// The probe covers the predicate; this covers the selection, which is where the branch used to
/// vanish: `select` emits one landing PER BEAD, so a branch with none emitted none.
#[test]
fn a_journal_only_branch_lands_with_no_bead_and_a_mixed_one_still_needs_a_trailer() {
    let (_tmp, main, _alpha) = land_repo("true");
    let bd = fake_bd(&main);
    let dead = &[("AIR_ATTRIBUTION_FALLBACK_BEFORE", "2000-01-01T00:00:00Z")];
    std::fs::write(
        main.join(".claude/air.json"),
        r#"{"verify_command": "true", "journal_dir": "docs/journal"}"#,
    )
    .unwrap();
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "chore: journal dir"]);

    // A FRESH worktree off main: `land_repo`'s alpha already carries work, which would make
    // the range genuinely mixed and prove the constraint rather than the permission.
    let scribe = main.parent().unwrap().join("scribe");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "worktree-scribe",
            scribe.to_str().unwrap(),
        ],
    );
    let scribe = scribe.canonicalize().unwrap();

    // A journal entry, with no `Bead:` trailer, as the journal's own nature implies.
    std::fs::create_dir_all(scribe.join("docs/journal")).unwrap();
    std::fs::write(scribe.join("docs/journal/scribe.md"), "what I hit\n").unwrap();
    git(&scribe, &["add", "docs/journal"]);
    git(
        &scribe,
        &["commit", "-q", "-m", "docs(journal): scribe, entries"],
    );
    assert_eq!(
        air_env(&scribe, &bd, &["record", "verify", "--", "true"], dead).0,
        0
    );

    let landable = |o: &str| -> serde_json::Value {
        let v: serde_json::Value = serde_json::from_str(o).unwrap();
        v["snapshot"]["landable"].clone()
    };
    let (_, out, _) = air_env(&main, &bd, &["--json", "status"], dead);
    let l = landable(&out);
    let rows = l.as_array().cloned().unwrap_or_default();
    assert_eq!(rows.len(), 1, "the journal branch is landable: {out}");
    assert_eq!(rows[0]["worker"], "scribe", "{out}");
    // It carries no bead, and says so rather than naming one it did not touch.
    assert!(rows[0]["bead"].is_null(), "carries no bead: {out}");
    assert!(rows[0]["blocked"].is_null(), "not blocked: {out}");

    // THE CONSTRAINT: one non-journal commit and the branch needs a trailer again.
    std::fs::write(scribe.join("work.txt"), "real work\n").unwrap();
    git(&scribe, &["add", "work.txt"]);
    git(
        &scribe,
        &["commit", "-q", "-m", "feat: work with no trailer"],
    );
    assert_eq!(
        air_env(&scribe, &bd, &["record", "verify", "--", "true"], dead).0,
        0
    );
    let (_, out, _) = air_env(&main, &bd, &["--json", "status"], dead);
    assert!(
        landable(&out).as_array().is_none_or(|v| v.is_empty()),
        "a mixed range still needs a bead: {out}"
    );
    // And the refusal names the journal case rather than only saying "declare a bead".
    // Asserted on `air land`, because that is the surface that renders `skipped`: neither
    // `air status --json` nor its text carries it, which is air-72t7 and not this bead.
    let (code, out, err) = air_env(&main, &bd, &["land", "--worker", "scribe"], dead);
    let refusal = format!("{out}{err}");
    assert_ne!(code, 0, "a mixed range is refused: {refusal}");
    assert!(
        refusal.contains("docs/journal"),
        "the refusal names the journal case and the directory: {refusal}"
    );
    assert!(
        refusal.contains("touches more than that"),
        "and says why this range is not it: {refusal}"
    );
}
