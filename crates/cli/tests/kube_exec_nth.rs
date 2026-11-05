//! `scripts/kube-exec-nth`, the program of this repository's per-session target prefixes
//! (air-oep): the Nth Running pod of a deployment, sorted by name, or one line naming the
//! session and the count. A fake `kubectl` on PATH stands in for the cluster.
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

/// A `kubectl` that answers the three calls the script makes. `get deployment` prints a
/// two-label selector, `get pods` prints `FAKE_PODS` (unsorted, so the sort shows), and
/// `exec` prints its argv one per line. `FAKE_FAIL=get` makes every `get` fail like an
/// expired token.
const FAKE_KUBECTL: &str = r#"#!/bin/sh
case " $* " in
    *" get deployment "*)
        [ "${FAKE_FAIL:-}" = get ] && { echo "error: You must be logged in to the server (Unauthorized)" >&2; exit 1; }
        printf 'app.kubernetes.io/name=x,part-of=dev,'
        ;;
    *" get pods "*)
        [ "${FAKE_FAIL:-}" = get ] && { echo "error: You must be logged in to the server (Unauthorized)" >&2; exit 1; }
        printf '%s' "$FAKE_PODS"
        ;;
    *" exec "*)
        printf '%s\n' "$@"
        ;;
    *)
        echo "fake kubectl: unexpected: $*" >&2
        exit 9
        ;;
esac
"#;

fn run(n: &str, session: &str, pods: &str, fail: &str) -> (i32, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("kubectl");
    std::fs::write(&fake, FAKE_KUBECTL).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = common::command("sh")
        .arg(repo_root().join("scripts/kube-exec-nth"))
        .args([
            "/kc", "ns", "dep", "ctr", n, session, "env", "PATH=/x", "hostname",
        ])
        .env("PATH", path)
        .env("FAKE_PODS", pods)
        .env("FAKE_FAIL", fail)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

const THREE: &str = "pod/dep-c\npod/dep-a\npod/dep-b\n";

#[test]
fn the_nth_pod_sorted_by_name_gets_the_command_through_the_selector_and_container() {
    let (code, out, err) = run("2", "worker-1", THREE, "");
    assert_eq!(code, 0, "{err}");
    let argv: Vec<&str> = out.lines().collect();
    assert_eq!(
        argv,
        [
            "--kubeconfig",
            "/kc",
            "-n",
            "ns",
            "exec",
            "-i",
            "pod/dep-b",
            "-c",
            "ctr",
            "--",
            "env",
            "PATH=/x",
            "hostname"
        ]
    );
    let (_, out, _) = run("1", "lane", THREE, "");
    assert!(out.contains("pod/dep-a\n"), "{out}");
    let (_, out, _) = run("3", "worker-2", THREE, "");
    assert!(out.contains("pod/dep-c\n"), "{out}");
}

#[test]
fn an_n_past_the_count_fails_with_one_line_naming_the_session_and_the_count() {
    let (code, out, err) = run("4", "worker-3", THREE, "");
    assert_eq!(code, 2);
    assert_eq!(out, "", "nothing ran");
    assert_eq!(
        err,
        "kube-exec-nth: session worker-3 wants replica 4 of dep; Running pods: 3\n"
    );
    let (code, _, err) = run("1", "lane", "", "");
    assert_eq!(code, 2);
    assert_eq!(
        err,
        "kube-exec-nth: session lane wants replica 1 of dep; Running pods: 0\n"
    );
}

#[test]
fn a_bad_n_and_a_failed_kubectl_fail_before_anything_runs() {
    for n in ["0", "x", "", "01"] {
        let (code, out, err) = run(n, "lane", THREE, "");
        assert_eq!(code, 2, "n={n:?}");
        assert_eq!(out, "");
        assert!(
            err.contains("session lane: N must be a whole number from 1"),
            "{err}"
        );
    }
    let (code, out, err) = run("1", "lane", THREE, "get");
    assert_eq!(code, 2);
    assert_eq!(out, "");
    assert!(
        err.contains("Unauthorized"),
        "kubectl's own line passes through: {err}"
    );
    assert!(
        err.ends_with("kube-exec-nth: session lane: cannot read deployment dep in namespace ns\n"),
        "{err}"
    );
}
