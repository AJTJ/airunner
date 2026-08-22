//! Where the ledger lives: `<main checkout>/.air/`, found via the git common dir so all
//! linked worktrees share one file (plan 0001 §2).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use crate::{LedgerError, Result};

/// Process-local memo of `git rev-parse` answers, keyed by (flag, cwd). One `air status`
/// asked git the same two questions 13 times (measured 2026-08-22, air-4vu: 17 git spawns,
/// ~0.5 s, for an empty repo); the answers cannot change for a live process (a checkout does
/// not move). Only successes are memoised, so a repo initialised later is still found.
fn rev_parse_memo(cwd: &Path, flag: &'static str) -> Result<PathBuf> {
    static MEMO: OnceLock<Mutex<HashMap<(&'static str, PathBuf), PathBuf>>> = OnceLock::new();
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (flag, cwd.to_path_buf());
    if let Some(hit) = memo.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return Ok(hit);
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--path-format=absolute", flag])
        .output()
        .map_err(|source| LedgerError::Io {
            path: cwd.to_path_buf(),
            source,
        })?;
    if !out.status.success() {
        return Err(LedgerError::NotARepo(cwd.to_path_buf()));
    }
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if let Ok(mut m) = memo.lock() {
        m.insert(key, path.clone());
    }
    Ok(path)
}

/// Resolve `<repo>/.air` for the repository containing `cwd`.
///
/// Uses `git rev-parse --git-common-dir`; for a linked worktree that is `<main>/.git`, for the
/// primary checkout it is `.git`. The ledger sits next to it, never inside `.git`.
pub fn air_dir_for(cwd: &Path) -> Result<PathBuf> {
    let common = rev_parse_memo(cwd, "--git-common-dir")?;
    let root = common
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| LedgerError::NotARepo(cwd.to_path_buf()))?;
    Ok(root.join(".air"))
}

/// The worker name for a checkout: the worktree directory name for linked worktrees, or
/// `main` for the primary checkout. Matches adopter's `.claude/worktrees/<name>` layout.
pub fn worker_name_for(cwd: &Path) -> Result<String> {
    let top = rev_parse_memo(cwd, "--show-toplevel")?;
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
