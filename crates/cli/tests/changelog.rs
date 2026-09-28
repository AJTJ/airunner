//! `scripts/changelog-assemble.sh`, the step `make changelog` runs before `make release`
//! (air-be6d): fragments in `changelog.d/` fold into `CHANGELOG.md` under `[Unreleased]`.

mod common;

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn assemble(args: &[&Path], dry: bool) -> (i32, String, String) {
    let mut c = common::command("sh");
    c.arg(repo_root().join("scripts/changelog-assemble.sh"));
    if dry {
        c.arg("--dry-run");
    }
    let out = c.args(args).output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

const LOG: &str = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- old added\n\n### Fixed\n\n\
- old fixed\n\n## [0.4.9]\n\n### Fixed\n\n- older\n";

#[test]
fn fragments_fold_under_their_heading_and_are_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let frags = dir.path().join("changelog.d");
    std::fs::create_dir(&frags).unwrap();
    let log = dir.path().join("CHANGELOG.md");
    std::fs::write(&log, LOG).unwrap();
    std::fs::write(frags.join("zz-1.md"), "Added a new thing\n").unwrap();
    std::fs::write(frags.join("zz-2.md"), "Fixed: the new fix\n  wraps here.\n").unwrap();
    std::fs::write(frags.join("README.md"), "not a fragment\n").unwrap();

    // Dry run: the result on stdout, nothing changed.
    let (code, out, err) = assemble(&[&frags, &log], true);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("### Added\n\n- a new thing\n- old added\n"),
        "{out}"
    );
    assert!(
        out.contains("### Fixed\n\n- the new fix\n  wraps here.\n- old fixed\n"),
        "{out}"
    );
    // Only [Unreleased] takes entries; the released section is untouched.
    assert!(
        out.ends_with("## [0.4.9]\n\n### Fixed\n\n- older\n"),
        "{out}"
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap(), LOG);
    assert!(frags.join("zz-1.md").exists());

    // For real: written, fragments gone, the README kept.
    let (code, _, err) = assemble(&[&frags, &log], false);
    assert_eq!(code, 0, "{err}");
    assert_eq!(std::fs::read_to_string(&log).unwrap(), out);
    assert!(!frags.join("zz-1.md").exists() && !frags.join("zz-2.md").exists());
    assert!(frags.join("README.md").exists());

    // A fragment with no known heading changes nothing.
    std::fs::write(frags.join("bad.md"), "Tweaked something\n").unwrap();
    let before = std::fs::read_to_string(&log).unwrap();
    let (code, _, err) = assemble(&[&frags, &log], false);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("does not start with Added"), "{err}");
    assert_eq!(std::fs::read_to_string(&log).unwrap(), before);
    assert!(frags.join("bad.md").exists());
}

/// This repo's own fragments assemble: every one lands in the dry run's output.
#[test]
fn this_repos_fragments_assemble() {
    let root = repo_root();
    let (code, out, err) = assemble(
        &[&root.join("changelog.d"), &root.join("CHANGELOG.md")],
        true,
    );
    assert_eq!(code, 0, "{err}");
    for e in std::fs::read_dir(root.join("changelog.d")).unwrap() {
        let p = e.unwrap().path();
        if p.file_name().is_some_and(|n| n == "README.md") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        let first = text.lines().next().unwrap_or_default();
        let entry = first.split_once(char::is_whitespace).unwrap().1;
        assert!(
            out.contains(&format!("- {entry}")),
            "{}: {out}",
            p.display()
        );
    }
}
