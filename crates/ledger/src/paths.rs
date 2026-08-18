//! Where the ledger lives: `<main checkout>/.air/`, found via the git common dir so all
//! linked worktrees share one file (plan 0001 §2).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{LedgerError, Result};

/// Resolve `<repo>/.air` for the repository containing `cwd`.
///
/// Uses `git rev-parse --git-common-dir`; for a linked worktree that is `<main>/.git`, for the
/// primary checkout it is `.git`. The ledger sits next to it, never inside `.git`.
pub fn air_dir_for(cwd: &Path) -> Result<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .map_err(|source| LedgerError::Io {
            path: cwd.to_path_buf(),
            source,
        })?;
    if !out.status.success() {
        return Err(LedgerError::NotARepo(cwd.to_path_buf()));
    }
    let common = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let common = PathBuf::from(common);
    let root = common
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| LedgerError::NotARepo(cwd.to_path_buf()))?;
    Ok(root.join(".air"))
}

/// The worker name for a checkout: the worktree directory name for linked worktrees, or
/// `main` for the primary checkout. Matches adopter's `.claude/worktrees/<name>` layout.
pub fn worker_name_for(cwd: &Path) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--path-format=absolute", "--show-toplevel"])
        .output()
        .map_err(|source| LedgerError::Io {
            path: cwd.to_path_buf(),
            source,
        })?;
    if !out.status.success() {
        return Err(LedgerError::NotARepo(cwd.to_path_buf()));
    }
    let top = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let common = air_dir_for(cwd)?;
    let main_root = common.parent().map(Path::to_path_buf).unwrap_or_default();
    if top == main_root {
        return Ok("main".to_string());
    }
    Ok(top
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string()))
}
