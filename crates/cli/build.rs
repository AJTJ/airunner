//! Embed the commit this binary was built from, so `air install` can name both sides when it
//! refuses a downgrade (air-w9d).
//!
//! `Installed.air_version` was `CARGO_PKG_VERSION`, which has been `0.0.1` in every build Air
//! has ever produced, while its own doc comment called it "the `air --version` that last wrote
//! this file". It cannot tell two binaries apart, which is the whole question here: the
//! installed `air` was a week old for an entire round and every command reported success.
//!
//! **mtime is not usable and this deliberately does not use it.** `cargo install --path` copies
//! the artifact with `fs::copy`, which preserves the source timestamp on macOS, so a freshly
//! installed binary can carry a two-hour-old mtime — wrong in the safe-looking direction.
//!
//! Borrowed idiom, not a dependency: `vergen`, `shadow-rs` and `git-version` all do this by
//! running git in a build script and emitting `cargo::rustc-env`. For one string that is ~30
//! lines against a third-party build-time dependency, so the idea is taken and the crate is
//! not (CLAUDE.md, "Steal avidly" and "research must show why not").
//!
//! Never fails the build. A source tarball with no `.git`, or no `git` on PATH, yields
//! `unknown`, and every consumer treats that as "cannot tell" rather than as a mismatch.

use std::process::Command;

fn main() {
    // Rebuild when HEAD moves. `.git` may be a FILE in a worktree, so its parent is not
    // necessarily a directory we can point at; watching both paths is harmless when absent.
    println!("cargo::rerun-if-changed=../../.git/HEAD");
    println!("cargo::rerun-if-changed=../../.git");
    println!("cargo::rerun-if-env-changed=AIR_BUILD");

    // Every skill this repo carries, for the selftest probe that refuses an installed skill
    // naming one `air install` does not ship (air-rr98). Read from the directory rather than
    // typed into a list, so a skill added here is covered without anyone remembering to.
    println!("cargo::rerun-if-changed=../../.claude/skills");
    let skills: Vec<String> = std::fs::read_dir("../../.claude/skills")
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    println!("cargo::rustc-env=AIR_SKILL_NAMES={}", skills.join(","));

    surface_fold();

    // An explicit override wins, for a packager that builds outside a checkout.
    if let Ok(v) = std::env::var("AIR_BUILD")
        && !v.trim().is_empty()
    {
        println!("cargo::rustc-env=AIR_BUILD={}", v.trim());
        return;
    }

    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!s.is_empty()).then_some(s)
    };

    let build = match git(&["rev-parse", "--short=8", "HEAD"]) {
        // A dirty tree is a build nobody can reproduce from a sha, so it says so.
        Some(sha) => match git(&["status", "--porcelain", "--untracked-files=no"]) {
            Some(_) => format!("{sha}-dirty"),
            None => sha,
        },
        None => "unknown".to_string(),
    };
    println!("cargo::rustc-env=AIR_BUILD={build}");
}

/// Fold `surface.d/NNNN-<id>.rs`, one `SurfaceChange { … }` expression per file, into
/// `$OUT_DIR/surface.rs` as an array of `include!`s in filename order (air-38q). Every bead that
/// changed the surface used to append at the end of one array in `install.rs`, so two beads in a
/// round conflicted at the same line: four lane drops and one bad hand resolution in one
/// session, 2026-11-03. Same fix as `changelog.d/` (air-be6d), run at build time because the
/// surface is compiled in. Each fragment is included by its own path, so a malformed one is a
/// rustc error naming that file. Fails the build, naming the directory, when it is missing or
/// holds no fragment: an empty surface would install silently as "nothing to tell you".
fn surface_fold() {
    // A build script fails by saying why and exiting; cargo prints the message under
    // "error: failed to run custom build command".
    fn fail(why: &str) -> ! {
        eprintln!("{why}");
        std::process::exit(1)
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("surface.d");
    println!("cargo::rerun-if-changed={}", dir.display());
    let entries = match std::fs::read_dir(&dir) {
        Ok(d) => d,
        Err(e) => fail(&format!("surface.d: {}: {e}", dir.display())),
    };
    let mut files: Vec<std::path::PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    if files.is_empty() {
        fail(&format!(
            "surface.d: {} holds no *.rs fragment; see its README.md",
            dir.display()
        ));
    }
    let mut out = String::from("&[\n");
    for f in &files {
        println!("cargo::rerun-if-changed={}", f.display());
        // Doubled backslashes for a Windows path inside a string literal; a no-op elsewhere.
        out.push_str(&format!(
            "    include!(\"{}\"),\n",
            f.display().to_string().replace('\\', "\\\\")
        ));
    }
    out.push(']');
    let Ok(out_dir) = std::env::var("OUT_DIR") else {
        fail("OUT_DIR is unset; this runs under cargo");
    };
    let target = std::path::Path::new(&out_dir).join("surface.rs");
    if let Err(e) = std::fs::write(&target, out) {
        fail(&format!("{}: {e}", target.display()));
    }
}
