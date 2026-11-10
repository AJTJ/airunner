//! `scripts/trial/trust-copy.py`, the part of `make trial` that marks the scratch copy trusted
//! in Claude Code's `.claude.json` (air-zau): one key for one path, under the path's physical
//! form, with everything else in the file as it was, through a temp file and a rename. Every
//! run here uses a temporary `CLAUDE_CONFIG_DIR`; the real file is never touched.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/trial/trust-copy.py")
        .canonicalize()
        .unwrap()
}

/// `(exit, stdout, stderr)` of the script for `dir`, with `config` as `CLAUDE_CONFIG_DIR`.
fn run(config: &Path, dir: &Path) -> (i32, String, String) {
    let out = common::command("python3")
        .arg(script())
        .arg(dir)
        .env("CLAUDE_CONFIG_DIR", config)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn read(config: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(config.join(".claude.json")).unwrap()).unwrap()
}

/// A `.claude.json` with the keys Claude Code keeps beside `projects`, another project, and
/// an entry for the copy with fields of its own: after the write only the one key differs.
#[test]
fn the_one_key_is_written_and_every_other_key_is_as_it_was() {
    let config = tempfile::tempdir().unwrap();
    let copy = tempfile::tempdir().unwrap();
    let physical = copy.path().canonicalize().unwrap();
    let key = physical.display().to_string();
    let file = config.path().join(".claude.json");
    let before = serde_json::json!({
        "numStartups": 7,
        "theme": "dark",
        "oauthAccount": {"emailAddress": "someone@example.com"},
        "projects": {
            "/home/someone/other": {"hasTrustDialogAccepted": true, "allowedTools": ["Bash(ls)"]},
            key.clone(): {"allowedTools": [], "history": [{"display": "hi"}]}
        }
    });
    std::fs::write(&file, serde_json::to_string_pretty(&before).unwrap()).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    // The argument is the path as `$TMPDIR` gives it; the key is its physical form.
    let (code, out, err) = run(config.path(), copy.path());
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("trusted") && out.contains(&key), "{out}");
    let after = read(config.path());
    assert_eq!(
        after["projects"][&key]["hasTrustDialogAccepted"], true,
        "{after}"
    );
    // Everything but that one key is equal, including the entry's own other fields.
    let mut expected = before.clone();
    expected["projects"][&key]["hasTrustDialogAccepted"] = serde_json::Value::Bool(true);
    assert_eq!(after, expected);
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // No temp file left beside it.
    let names: Vec<String> = std::fs::read_dir(config.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, [".claude.json"], "{names:?}");

    // Already trusted: nothing is rewritten, byte for byte.
    let bytes = std::fs::read(&file).unwrap();
    let (code, out, _) = run(config.path(), copy.path());
    assert_eq!(code, 0);
    assert!(out.contains("already trusted"), "{out}");
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
}

/// A machine with no `.claude.json` yet gets one holding the single entry, mode 600; a file
/// that is not JSON is left as it is and the exit is 1.
#[test]
fn an_absent_file_is_created_and_a_broken_one_is_left_alone() {
    let config = tempfile::tempdir().unwrap();
    let copy = tempfile::tempdir().unwrap();
    let key = copy.path().canonicalize().unwrap().display().to_string();
    let (code, out, err) = run(config.path(), copy.path());
    assert_eq!(code, 0, "{out}{err}");
    let after = read(config.path());
    assert_eq!(
        after,
        serde_json::json!({"projects": {key: {"hasTrustDialogAccepted": true}}})
    );
    let file = config.path().join(".claude.json");
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );

    std::fs::write(&file, "{not json").unwrap();
    let (code, _, err) = run(config.path(), copy.path());
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("is not JSON") && err.contains("left as it is"),
        "{err}"
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{not json");
}
