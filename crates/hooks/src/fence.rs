//! The worktree fence (air-8gj): an Edit/Write whose resolved path leaves the session's
//! worktree is denied. Pure over two paths; the hook resolves and calls.
//!
//! What it replaces: Claude Code's `--worktree` isolation, which the adopter's record shows
//! stopped no observed write to the main checkout and cost 455 refusals in five days, 388
//! (88%) with no git token in the command (an adopter's note of 2026-09-06). The one gap that isolation did close and nothing else did is a hand-written
//! `../../main/<path>` in a file tool, which is exactly this check. A Bash `cd ../..` is out of
//! scope on purpose: the harness never caught that either, and the repo's cwd-scoped
//! command guard is where it belongs.
//!
//! Removal: when the harness keys its isolation on the cwd rather than the flag, or when a
//! round records zero outside-worktree edit denials AND the owner prefers the harness block.

use std::path::{Path, PathBuf};

/// Resolve `path` for comparison: the deepest existing ancestor canonicalised (symlinks
/// followed), the rest appended as written. A file being created does not exist yet; its
/// parent usually does. A path with no existing ancestor comes back as given.
pub fn resolve(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if let Ok(c) = existing.canonicalize() {
            let mut out = c;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(p), Some(name)) => {
                rest.push(name);
                existing = p;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Does a resolved path leave `worktree`? Pure: both must already be resolved the same way.
pub fn leaves(resolved: &Path, worktree: &Path) -> bool {
    !resolved.starts_with(worktree)
}

/// The denial, naming the path and the worktree, or `None` when the edit stays inside.
pub fn denial(path: &Path, worktree: &Path) -> Option<String> {
    let resolved = resolve(path);
    leaves(&resolved, worktree).then(|| {
        format!(
            "air: refusing an edit outside this session's worktree: {} resolves to {}, which \
             is not under {}. A worker edits its own worktree only (air-8gj); the main \
             checkout is the coordinator's.",
            path.display(),
            resolved.display(),
            worktree.display()
        )
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn inside_stays_and_outside_leaves() {
        let wt = Path::new("/r/.claude/worktrees/w");
        assert!(!leaves(Path::new("/r/.claude/worktrees/w/src/a.rs"), wt));
        assert!(leaves(Path::new("/r/src/a.rs"), wt));
        // A sibling whose name merely starts with the worktree's is outside.
        assert!(leaves(Path::new("/r/.claude/worktrees/w2/src/a.rs"), wt));
    }

    #[test]
    fn resolve_follows_the_deepest_existing_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap();
        // A file that does not exist yet resolves through its existing parent.
        let new = dir.path().join("new.rs");
        assert_eq!(resolve(&new), real.join("new.rs"));
        // Dot segments through an existing parent are collapsed by canonicalising it.
        let dotted = dir.path().join("sub").join("..").join("x.rs");
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        assert_eq!(resolve(&dotted), real.join("x.rs"));
        // Nothing of it exists: returned as given, never a panic.
        assert_eq!(
            resolve(Path::new("/nonexistent-zz/a/b")),
            PathBuf::from("/nonexistent-zz/a/b")
        );
        assert!(denial(&new, &real).is_none());
        assert!(denial(&real.join("..").join("elsewhere.rs"), &real).is_some());
    }
}
