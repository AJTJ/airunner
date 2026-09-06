//! tmux naming for worker panes (air-5lg).
//!
//! `tmux ls` is machine-wide, not per directory. With the adopter's fleet and ai_runner's
//! running at once the list said nothing about which project a pane belonged to and the owner
//! had to attach to find out (owner, 2026-08-22). So Air names the session
//! `<project>-<worker>` and puts the name in `air status`, and `air claim` renames the window
//! to the bead so the list also says what the lane is doing now.
//!
//! Every function here either is pure or shells out to `tmux` and treats failure as "no
//! tmux": naming is a convenience and must never fail a launch or a claim.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The main checkout for `repo` (the ledger's directory, minus `.air`).
fn main_checkout(repo: &Path) -> PathBuf {
    air_ledger::paths::air_dir_for(repo)
        .ok()
        .and_then(|d| d.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| repo.to_path_buf())
}

/// The project's beads prefix: `issue-prefix:` in `.beads/config.yaml` when set, else the
/// Dolt database name in `.beads/metadata.json` (what bd 1.2.2 derives ids from: `air` here,
/// `fd` in the adopter), else the main checkout's directory name.
pub fn project_prefix(repo: &Path) -> String {
    let main = main_checkout(repo);
    if let Some(p) = config_prefix(&main.join(".beads/config.yaml")) {
        return p;
    }
    if let Some(p) = metadata_prefix(&main.join(".beads/metadata.json")) {
        return p;
    }
    main.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "air".into())
}

/// `issue-prefix: "foo"` from a beads config. Commented-out lines do not count, which is how
/// the setting ships (`# issue-prefix: ""`), so this usually falls through to the metadata.
fn config_prefix(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix("issue-prefix:"))
        .map(|v| v.trim().trim_matches(['"', '\'']).to_string())
        .filter(|v| !v.is_empty())
}

fn metadata_prefix(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("dolt_database")?
        .as_str()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

/// The tmux session name for a worker. tmux reads `:` as a window separator and `.` as a pane
/// separator inside a target, and whitespace makes `attach -t` awkward, so those become `-`.
pub fn session_name(project: &str, worker: &str) -> String {
    let clean = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_whitespace() || ":.".contains(c) {
                    '-'
                } else {
                    c
                }
            })
            .collect()
    };
    // A leading `-` reads as a flag to every `tmux -t` call, and a dotfile directory name
    // (temp dirs, `.worktrees`) produces one once `.` is replaced.
    let trim = |s: String| s.trim_matches('-').to_string();
    let (p, w) = (trim(clean(project.trim())), trim(clean(worker.trim())));
    if p.is_empty() { w } else { format!("{p}-{w}") }
}

/// `-L <socket>` when `AIR_TMUX_SOCKET` is set, so tests never touch the user's tmux server.
fn socket_args() -> Vec<String> {
    match std::env::var("AIR_TMUX_SOCKET") {
        Ok(s) if !s.is_empty() => vec!["-L".into(), s],
        _ => Vec::new(),
    }
}

/// Run tmux, quietly. None when tmux is absent or the command failed (no server yet, no such
/// session): naming never fails the caller.
fn tmux(args: &[String]) -> Option<String> {
    let mut v = socket_args();
    v.extend(args.iter().cloned());
    let out = Command::new("tmux")
        .args(&v)
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Live tmux session names. Empty when tmux is absent or no server is running.
pub fn sessions() -> Vec<String> {
    tmux(&[
        "list-sessions".into(),
        "-F".into(),
        "#{session_name}".into(),
    ])
    .map(|s| s.lines().map(str::trim).map(str::to_string).collect())
    .unwrap_or_default()
}

/// Rename the worker's tmux window so `tmux ls` shows what the lane is doing. `label` empty
/// resets it to the worker name. Silent when there is no such session.
pub fn set_window_label(project: &str, worker: &str, label: &str) {
    let session = session_name(project, worker);
    let name = if label.trim().is_empty() {
        worker.to_string()
    } else {
        window_label(label)
    };
    let _ = tmux(&["rename-window".into(), "-t".into(), session, name]);
}

/// One tmux window name: no newlines, and short enough to leave room in `tmux ls`.
pub fn window_label(text: &str) -> String {
    let flat: String = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['#', '\t'], " ");
    match flat.char_indices().nth(40) {
        Some((i, _)) => format!("{}…", flat.get(..i).unwrap_or(&flat).trim_end()),
        None => flat,
    }
}

/// The worker name `air worker` picks when none is given: `w1`, `w2`, … skipping any name a
/// worktree or a live tmux session already uses. A worker outlives its bead (tty-fix did six),
/// so the bead never belongs in the name (owner ruling, 2026-08-22).
pub fn next_free_worker_name(taken: &[String], project: &str) -> String {
    free_worker_name(taken, &sessions(), project)
}

/// The decision, with the live sessions passed in rather than asked for (air-7ah). Its test
/// used to call `sessions()`, which shells to tmux and reads the ambient `AIR_TMUX_SOCKET`, so
/// it would start failing the moment anyone ran `air worker` with no name and left an `air-w1`
/// pane behind. A test that reads its answer out of the machine is not a test of the rule.
pub fn free_worker_name(taken: &[String], live: &[String], project: &str) -> String {
    let used: std::collections::BTreeSet<&str> = taken.iter().map(String::as_str).collect();
    (1..=999)
        .map(|n| format!("w{n}"))
        .find(|n| {
            !used.contains(n.as_str()) && !live.iter().any(|s| *s == session_name(project, n))
        })
        .unwrap_or_else(|| "w1".into())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn session_name_joins_project_and_worker_and_strips_tmux_separators() {
        assert_eq!(session_name("air", "alpha"), "air-alpha");
        assert_eq!(session_name("zz", "first-agent"), "zz-first-agent");
        // `:` and `.` address windows and panes; whitespace breaks `attach -t`.
        assert_eq!(session_name("my.proj", "a b:c"), "my-proj-a-b-c");
        // No prefix resolved: the worker name alone, as before air-5lg.
        assert_eq!(session_name("", "alpha"), "alpha");
    }

    #[test]
    fn prefix_prefers_the_config_then_the_dolt_database_then_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("ai_runner");
        std::fs::create_dir_all(repo.join(".beads")).unwrap();
        // Neither file: the main checkout's directory name.
        assert_eq!(project_prefix(&repo), "ai_runner");
        // The shipped config comments the setting out, so it must not count.
        std::fs::write(
            repo.join(".beads/config.yaml"),
            "# issue-prefix: \"\"\njson: false\n",
        )
        .unwrap();
        assert_eq!(project_prefix(&repo), "ai_runner");
        std::fs::write(
            repo.join(".beads/metadata.json"),
            r#"{"backend":"dolt","dolt_database":"air"}"#,
        )
        .unwrap();
        assert_eq!(project_prefix(&repo), "air");
        std::fs::write(repo.join(".beads/config.yaml"), "issue-prefix: \"zz\"\n").unwrap();
        assert_eq!(project_prefix(&repo), "zz");
    }

    #[test]
    fn window_label_is_one_short_line() {
        assert_eq!(window_label("air-5lg  tmux\nnames"), "air-5lg tmux names");
        let long = window_label(&format!("air-5lg {}", "x".repeat(60)));
        assert!(long.chars().count() <= 41, "{long}");
        assert!(long.ends_with('…'), "{long}");
    }

    #[test]
    fn next_free_worker_name_skips_taken_names() {
        let none: Vec<String> = vec![];
        assert_eq!(free_worker_name(&[], &none, "air"), "w1");
        assert_eq!(
            free_worker_name(&["w1".into(), "w2".into(), "alpha".into()], &none, "air"),
            "w3"
        );
        // A live pane counts as taken, and it is supplied rather than read off the machine
        // (air-7ah): calling `sessions()` here made the answer depend on whatever tmux
        // happened to be running.
        assert_eq!(
            free_worker_name(&[], &["air-w1".to_string(), "zz-w2".to_string()], "air"),
            "w2",
            "another project's pane does not reserve our name"
        );
    }
}
