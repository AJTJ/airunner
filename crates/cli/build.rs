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
