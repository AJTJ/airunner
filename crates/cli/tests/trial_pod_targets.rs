//! `scripts/trial/pod-targets.py`, the part of `TRIAL_PODS=1 make trial` that writes the trial
//! copy's `.claude/air.local.json` (air-1bn): five targets copied from the owner's own
//! kube-exec-nth entry, each on its own replica with its own path, written only once replica 1
//! has answered `hostname`. A fake `kubectl` on PATH stands in for the cluster, as in
//! `kube_exec_nth.rs`.
#![allow(clippy::unwrap_used)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// `exec` answers `hostname` with the pod's name; `FAKE_FAIL=get` makes every `get` fail like
/// an expired token.
const FAKE_KUBECTL: &str = r#"#!/bin/sh
case " $* " in
    *" get deployment "*)
        [ "${FAKE_FAIL:-}" = get ] && { echo "error: You must be logged in to the server (Unauthorized)" >&2; exit 1; }
        printf 'app=x,'
        ;;
    *" get pods "*)
        [ "${FAKE_FAIL:-}" = get ] && { echo "error: You must be logged in to the server (Unauthorized)" >&2; exit 1; }
        printf 'pod/dep-b\npod/dep-a\n'
        ;;
    *" exec "*)
        echo "host-of-${7#pod/}"
        ;;
    *)
        echo "fake kubectl: unexpected: $*" >&2
        exit 9
        ;;
esac
"#;

const OWNERS_OVERLAY: &str = r#"{
  "coordinator_allow": ["Bash(kubectl *)"],
  "targets": {
    "vm": {"kind": "ssh", "host": "h", "user": "u", "path": "/home/u/air/vm"},
    "lane": {
      "exec": ["/elsewhere/scripts/kube-exec-nth", "/kc", "ns", "dep", "ctr", "2", "lane", "env", "PATH=/x"],
      "path": "/home/u/air/airunner"
    }
  }
}
"#;

/// `(exit, stdout, stderr)` of the script against `source`, with the overlay going to
/// `<dir>/out.json`.
fn run(dir: &Path, source: &str, fail: &str) -> (i32, String, String) {
    let fake = dir.join("kubectl");
    std::fs::write(&fake, FAKE_KUBECTL).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let src = dir.join("source.json");
    std::fs::write(&src, source).unwrap();
    let path = format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = common::command("python3")
        .arg(repo_root().join("scripts/trial/pod-targets.py"))
        .arg(&src)
        .arg(script())
        .arg(dir.join("out.json"))
        .env("PATH", path)
        .env("FAKE_FAIL", fail)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// The kube-exec-nth the overlay names and the probe runs: this checkout's.
fn script() -> String {
    repo_root()
        .join("scripts/kube-exec-nth")
        .to_string_lossy()
        .to_string()
}

fn python3() -> bool {
    if common::command("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("SKIP: python3 not installed");
        return false;
    }
    true
}

fn target(n: u32, session: &str) -> serde_json::Value {
    serde_json::json!({
        "exec": [script(), "/kc", "ns", "dep", "ctr", n.to_string(), session, "env", "PATH=/x"],
        "path": format!("/home/u/air/trial/{session}"),
    })
}

#[test]
fn five_targets_are_copied_from_the_owners_entry_once_replica_one_answers() {
    if !python3() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (code, out, err) = run(dir.path(), OWNERS_OVERLAY, "");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.starts_with("trial: pods: replica 1 answers as host-of-dep-a; "),
        "sorted first pod, through the coordinator's prefix: {out}"
    );
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("out.json")).unwrap())
            .unwrap();
    assert_eq!(
        written,
        serde_json::json!({"targets": {
            "coordinator": target(1, "coordinator"),
            "lane": target(2, "lane"),
            "worker-1": target(3, "worker-1"),
            "worker-2": target(4, "worker-2"),
            "worker-3": target(5, "worker-3"),
        }}),
        "this checkout's script, the owner's four values and tail, N+2, one path each, and no other key"
    );
}

#[test]
fn a_cluster_that_does_not_answer_stops_with_its_reason_and_writes_nothing() {
    if !python3() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (code, out, err) = run(dir.path(), OWNERS_OVERLAY, "get");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(
        err.starts_with("trial: pods: replica 1 did not answer `hostname` through "),
        "{err}"
    );
    assert!(
        err.ends_with(
            "\n  error: You must be logged in to the server (Unauthorized)\n  \
             kube-exec-nth: session coordinator: cannot read deployment dep in namespace ns\n"
        ),
        "kubectl's own line and the script's: {err}"
    );
    assert!(
        !dir.path().join("out.json").exists(),
        "no overlay on failure"
    );
    assert!(!dir.path().join("out.json.new").exists());
}

#[test]
fn a_source_without_a_kube_exec_nth_entry_is_refused_before_the_cluster_is_asked() {
    if !python3() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source =
        r#"{"targets": {"lane": {"kind": "ssh", "host": "h", "user": "u", "path": "/p"}}}"#;
    let (code, out, err) = run(dir.path(), source, "get");
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(
        err.ends_with("has no target whose exec runs kube-exec-nth with its six arguments\n"),
        "{err}"
    );
    assert!(!dir.path().join("out.json").exists());
}
