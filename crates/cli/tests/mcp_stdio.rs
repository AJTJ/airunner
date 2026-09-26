//! `air mcp` end to end over piped stdio: handshake, a tool call, a resource read, survival
//! of a garbage line, a channel push for a seeded idle-with-claim session, and a clean exit on EOF.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

/// Seed a session row that has been `idle` for an hour while holding a claim (`stuck` was the
/// seeded state until air-12k deleted it).
/// bd is not under test here; a missing binary fails in microseconds where a real `bd` in a
/// non-beads directory cost 0.25 to 0.5 s per call, three calls per `status` (air-4vu).
const NO_BD: &str = "/nonexistent/bd";

fn seed_idle_with_claim(repo: &Path) {
    let out = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(repo)
        .arg("status")
        .env("AIR_BD_BIN", NO_BD)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(out.status.success());
    let conn = rusqlite::Connection::open(repo.join(".air/ledger.db")).unwrap();
    conn.execute(
        "INSERT INTO sessions (session_id, worker, state, detail, changed_at, started_at, role) \
         VALUES ('s1','main','idle',NULL,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z','coordinator')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO claims (bead, worker, claimed_at) VALUES ('x-1','main','2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
}

#[test]
fn mcp_over_stdio_serves_tools_resources_and_pushes_channel_events() {
    let dir = scratch_repo();
    let repo = dir.path().canonicalize().unwrap();
    seed_idle_with_claim(&repo);

    let mut child = Command::new(env!("CARGO_BIN_EXE_air"))
        .arg("--repo")
        .arg(&repo)
        .arg("mcp")
        .current_dir(&repo)
        // The first tick runs before the first sleep, so the seeded idle session is pushed
        // at startup whatever the interval. A short interval only makes the poll thread
        // (3 bd + ~5 git spawns per tick) fight the requests below for the stdout lock:
        // 50 ms cost 3.4 s per run (air-4vu, 2026-08-22). 5 s: no second tick in a run.
        .env("AIR_CHANNEL_POLL_MS", "5000")
        .env("AIR_BD_BIN", NO_BD)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    // Lines arrive on a channel from a reader thread, so every wait has a real deadline: a
    // blocking `read_line` on the test thread hung for minutes when the server stopped answering.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    // Collect lines until a predicate matches, bounded in time; notifications may interleave.
    // Unmatched lines are kept: the poll thread's first tick (the seeded idle session)
    // usually lands before the `initialize` reply, and dropping it meant waiting a whole
    // poll interval for the second tick (air-4vu, 2026-08-22).
    let mut pending: Vec<serde_json::Value> = Vec::new();
    let mut next_matching = |pred: &dyn Fn(&serde_json::Value) -> bool| -> serde_json::Value {
        if let Some(i) = pending.iter().position(pred) {
            return pending.remove(i);
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = match rx.recv_timeout(left) {
                Ok(line) => line,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    panic!("timed out waiting for a matching line")
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("server closed stdout early")
                }
            };
            let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
            if pred(&v) {
                return v;
            }
            pending.push(v);
        }
    };

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"t","version":"0"}}}}}}"#).unwrap();
    let init = next_matching(&|v| v["id"] == 1);
    assert!(init["result"]["capabilities"]["experimental"]["claude/channel"].is_object());
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();

    // A garbage line must produce a parse error and not kill the server.
    writeln!(stdin, "this is not json").unwrap();
    let perr = next_matching(&|v| v.get("error").is_some());
    assert_eq!(perr["error"]["code"], -32700);

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"air_capture","arguments":{{"text":"from mcp"}}}}}}"#).unwrap();
    let cap = next_matching(&|v| v["id"] == 2);
    assert_eq!(cap["result"]["isError"], false, "{cap}");
    let text = cap["result"]["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["inbox_depth"], 1);

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{{"uri":"air://inbox"}}}}"#
    )
    .unwrap();
    let res = next_matching(&|v| v["id"] == 3);
    let inbox: serde_json::Value =
        serde_json::from_str(res["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
    // Captures and landings in one shape since air-6p5.
    assert_eq!(inbox["captures"][0]["text"], "from mcp");

    // The poll thread must have pushed the seeded idle session as a channel event.
    let ev = next_matching(&|v| v["method"] == "notifications/claude/channel");
    assert_eq!(ev["params"]["meta"]["kind"], "idle_with_claim");
    assert_eq!(ev["params"]["meta"]["worker"], "main");

    // A tool argument error is a JSON-RPC error, not a crash.
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"air_triage","arguments":{{"id":"x"}}}}}}"#).unwrap();
    let bad = next_matching(&|v| v["id"] == 4);
    assert_eq!(bad["error"]["code"], -32602);

    // Memory canary: 200 in-process requests must not grow the server by 8 MB, so it sees a
    // leak of about 40 KB per request or more. It was 1500 (air-4vu); cut on 2026-09-26 when a
    // loaded machine made the run slow enough to look hung.
    let rss = |pid: u32| -> u64 {
        let out = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .unwrap_or(0)
    };
    let pid = child.id();
    for i in 0..50 {
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":{},"method":"tools/list"}}"#,
            1000 + i
        )
        .unwrap();
        let _ = next_matching(&|v| v["id"] == 1000 + i);
    }
    let before = rss(pid);
    for i in 0..200 {
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":{},"method":"resources/list"}}"#,
            5000 + i
        )
        .unwrap();
        let _ = next_matching(&|v| v["id"] == 5000 + i);
    }
    let after = rss(pid);
    assert!(
        after <= before.saturating_add(8 * 1024),
        "rss grew from {before} KB to {after} KB over 200 requests"
    );

    // EOF on stdin: clean exit, no orphan.
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "server did not exit on EOF");
        std::thread::sleep(Duration::from_millis(20));
    }
}
