//! `air install [--write]`: wire Air into a target repo's Claude Code config.
//!
//! What it touches, and only with `--write` (dry run prints the exact before/after):
//! - `<repo>/.claude/settings.json`: the hook entries, merged (never duplicated, never
//!   removing anything that is not ours).
//! - `<repo>/.mcp.json`: the `air` server (`air mcp`), merged the same way.
//! - `<repo>/.air/`: created; `roles.md` written from the copy embedded in this binary so the
//!   launchers can pass it with `--append-system-prompt-file`.
//! - `.gitignore`: advises if `.air/` is not ignored; does not edit it.
//!
//! It never touches the live fleet's state beyond these files, and it refuses to write when
//! the `air` on PATH is not this binary (enforcement rank 10: a worktree copy must not be
//! what the hooks resolve to).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cmd::emit;

/// The roles document, embedded so installs are self-contained.
pub const ROLES_MD: &str = include_str!("../../../../docs/rules/roles.md");

/// The coordinator's procedures, embedded and installed as skills in the target repo so every
/// coordinator carries the same reasoning, versioned with `air` (owner, 2026-08-21). The
/// same text is served as MCP prompts by `air mcp`.
pub const SKILLS: &[(&str, &str)] = &[
    (
        "air-decomposition",
        include_str!("../../../../.claude/skills/decomposition/SKILL.md"),
    ),
    (
        "air-phase-transitions",
        include_str!("../../../../.claude/skills/phase-transitions/SKILL.md"),
    ),
];

/// Rename the frontmatter `name:` so the installed copy does not collide with a repo's own
/// skill of the same name. Pure.
pub fn skill_with_name(text: &str, name: &str) -> String {
    let mut out = String::with_capacity(text.len().saturating_add(16));
    let mut in_front = false;
    let mut renamed = false;
    for (i, line) in text.lines().enumerate() {
        if i == 0 && line.trim() == "---" {
            in_front = true;
        } else if in_front && line.trim() == "---" {
            in_front = false;
        }
        if in_front && !renamed && line.starts_with("name:") {
            out.push_str(&format!("name: {name}\n"));
            renamed = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

const HOOK_TIMEOUT_SECS: u64 = 5;

/// The hook table (plan 0001 §5): one command for every event, short timeout.
pub fn hook_entries() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        ("SessionStart", None),
        ("PreToolUse", Some("Edit|Write|MultiEdit|Bash")),
        ("PostToolUse", Some("Edit|Write|MultiEdit|Bash")),
        ("PermissionRequest", None),
        ("PermissionDenied", None),
        ("PostToolUseFailure", None),
        ("Stop", None),
        ("SubagentStop", None),
        ("SessionEnd", None),
    ]
}

fn air_hook() -> Value {
    json!({"type": "command", "command": "air hook", "timeout": HOOK_TIMEOUT_SECS})
}

fn is_ours(h: &Value) -> bool {
    h.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c == "air hook" || c.ends_with("/air hook"))
}

/// Pure: merge our hook entries into a settings object. Idempotent.
pub fn merge_hooks(mut settings: Value) -> Value {
    if !settings.is_object() {
        settings = json!({});
    }
    let hooks = settings
        .as_object_mut()
        .map(|o| o.entry("hooks").or_insert_with(|| json!({})))
        .filter(|h| h.is_object());
    let Some(hooks) = hooks else {
        return settings;
    };
    for (event, matcher) in hook_entries() {
        let arr = hooks
            .as_object_mut()
            .map(|o| o.entry(event).or_insert_with(|| json!([])))
            .and_then(Value::as_array_mut);
        let Some(arr) = arr else { continue };
        let already = arr.iter().any(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_some_and(|hs| hs.iter().any(is_ours))
        });
        if already {
            continue;
        }
        let group = match matcher {
            Some(m) => json!({"matcher": m, "hooks": [air_hook()]}),
            None => json!({"hooks": [air_hook()]}),
        };
        arr.push(group);
    }
    settings
}

/// Pure: merge the `air` MCP server into a `.mcp.json` object. Idempotent.
pub fn merge_mcp(mut mcp: Value) -> Value {
    if !mcp.is_object() {
        mcp = json!({});
    }
    if let Some(servers) = mcp
        .as_object_mut()
        .map(|o| o.entry("mcpServers").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut)
    {
        servers
            .entry("air")
            .or_insert_with(|| json!({"command": "air", "args": ["mcp"]}));
    }
    mcp
}

fn read_json(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(json!({})),
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let mut s = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    s.push('\n');
    std::fs::write(path, s).map_err(|e| format!("{}: {e}", path.display()))
}

/// The `air` that PATH resolves to, if any.
fn air_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join("air"))
        .find(|p| p.is_file())
}

#[derive(Debug, serde::Serialize)]
struct Plan {
    repo: PathBuf,
    binary: PathBuf,
    path_binary: Option<PathBuf>,
    binary_ok: bool,
    settings_path: PathBuf,
    settings_changed: bool,
    mcp_path: PathBuf,
    mcp_changed: bool,
    air_dir: PathBuf,
    skills_dir: PathBuf,
    gitignore_has_air: bool,
    written: bool,
}

pub fn run(repo: &Path, write: bool, json: bool) -> i32 {
    let repo = match repo.canonicalize() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("air install: {}: {e}", repo.display());
            return 1;
        }
    };
    let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("air"));
    let path_binary = air_on_path();
    let binary_ok = match (&path_binary, binary.canonicalize()) {
        (Some(p), Ok(me)) => p.canonicalize().map(|p| p == me).unwrap_or(false),
        _ => false,
    };
    let settings_path = repo.join(".claude/settings.json");
    let mcp_path = repo.join(".mcp.json");
    let air_dir = repo.join(".air");
    let skills_dir = repo.join(".claude/skills");

    let before_settings = match read_json(&settings_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air install: {e}");
            return 1;
        }
    };
    let after_settings = merge_hooks(before_settings.clone());
    let before_mcp = match read_json(&mcp_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air install: {e}");
            return 1;
        }
    };
    let after_mcp = merge_mcp(before_mcp.clone());
    let gitignore_has_air = std::fs::read_to_string(repo.join(".gitignore"))
        .map(|s| {
            s.lines()
                .any(|l| matches!(l.trim(), ".air" | ".air/" | "/.air" | "/.air/"))
        })
        .unwrap_or(false);

    let mut plan = Plan {
        repo: repo.clone(),
        binary,
        path_binary,
        binary_ok,
        settings_path: settings_path.clone(),
        settings_changed: before_settings != after_settings,
        mcp_path: mcp_path.clone(),
        mcp_changed: before_mcp != after_mcp,
        air_dir: air_dir.clone(),
        skills_dir: skills_dir.clone(),
        gitignore_has_air,
        written: false,
    };

    if write {
        if !binary_ok {
            eprintln!(
                "air install: refusing to write: `air` on PATH is {} but this binary is {}. Install this binary on PATH first (cargo install --path crates/cli).",
                plan.path_binary
                    .as_deref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "absent".into()),
                plan.binary.display()
            );
            return 2;
        }
        let steps: Result<(), String> = (|| {
            if plan.settings_changed {
                write_json(&settings_path, &after_settings)?;
            }
            if plan.mcp_changed {
                write_json(&mcp_path, &after_mcp)?;
            }
            std::fs::create_dir_all(&air_dir).map_err(|e| format!("{}: {e}", air_dir.display()))?;
            std::fs::write(air_dir.join("roles.md"), ROLES_MD)
                .map_err(|e| format!("{}: {e}", air_dir.display()))?;
            for (name, text) in SKILLS {
                let dir = skills_dir.join(name);
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                std::fs::write(dir.join("SKILL.md"), skill_with_name(text, name))
                    .map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            Ok(())
        })();
        if let Err(e) = steps {
            eprintln!("air install: {e}");
            return 1;
        }
        plan.written = true;
    }

    emit(json, &plan, || {
        let mut s = String::new();
        s.push_str(&format!("repo:     {}\n", plan.repo.display()));
        s.push_str(&format!(
            "binary:   {} ({})\n",
            plan.binary.display(),
            if plan.binary_ok {
                "is the `air` on PATH"
            } else {
                "NOT the `air` on PATH; fix before --write"
            }
        ));
        s.push_str(&format!(
            "settings: {} ({})\n",
            plan.settings_path.display(),
            if plan.settings_changed {
                "will add air hooks"
            } else {
                "already wired"
            }
        ));
        if plan.settings_changed && !plan.written {
            s.push_str(&format!(
                "--- after:\n{}\n",
                serde_json::to_string_pretty(&after_settings).unwrap_or_default()
            ));
        }
        s.push_str(&format!(
            "mcp:      {} ({})\n",
            plan.mcp_path.display(),
            if plan.mcp_changed {
                "will add the air server"
            } else {
                "already wired"
            }
        ));
        s.push_str(&format!(
            "ledger:   {}/ (roles.md written here)\n",
            plan.air_dir.display()
        ));
        if !plan.gitignore_has_air {
            s.push_str("advice:   add `.air/` to .gitignore\n");
        }
        s.push_str(if plan.written {
            "written.\n"
        } else {
            "dry run; re-run with --write to apply.\n"
        });
        s
    });
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// The embedded roles prose is `include_str!` of docs/rules/roles.md, so the two cannot
    /// drift; this pins that, plus the lines the round added: run-to-completion and the
    /// coordinator reach/landing facts (air-arq), bd's per-type sections (air-8zz).
    #[test]
    fn embedded_roles_match_the_file_and_carry_the_round_lines() {
        let on_disk = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/rules/roles.md"
        ))
        .unwrap();
        assert_eq!(ROLES_MD, on_disk);
        assert!(ROLES_MD.contains("A hand-over is not a stop."));
        assert!(ROLES_MD.contains("Workers are reached with `SendMessage`"));
        assert!(ROLES_MD.contains("Landing is the coordinator's: `air land --all`"));
        assert!(ROLES_MD.contains("bug `## Steps to Reproduce` + `## Acceptance Criteria`"));
        assert!(ROLES_MD.contains("epic `## Success"));
    }

    #[test]
    fn merge_hooks_is_idempotent_and_preserves_others() {
        let existing = json!({
            "permissions": {"allow": ["Bash(ls *)"]},
            "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "rtk hook"}]}]}
        });
        let once = merge_hooks(existing);
        let twice = merge_hooks(once.clone());
        assert_eq!(once, twice);
        assert_eq!(once["permissions"]["allow"][0], "Bash(ls *)");
        let pre = once["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "{pre:?}");
        assert_eq!(pre[0]["hooks"][0]["command"], "rtk hook");
        assert_eq!(pre[1]["hooks"][0]["command"], "air hook");
        assert_eq!(pre[1]["matcher"], "Edit|Write|MultiEdit|Bash");
        assert!(once["hooks"]["SessionStart"][0].get("matcher").is_none());
        for (event, _) in hook_entries() {
            assert!(once["hooks"][event].is_array(), "{event}");
        }
    }

    #[test]
    fn skill_rename_touches_only_the_frontmatter_name() {
        let t = "---\nname: decomposition\ndescription: x\n---\n# Title\nname: not frontmatter\n";
        let r = skill_with_name(t, "air-decomposition");
        assert!(r.starts_with("---\nname: air-decomposition\ndescription: x\n---\n"));
        assert!(r.contains("name: not frontmatter"));
        assert_eq!(SKILLS.len(), 2);
        assert!(SKILLS[0].1.contains("Provenance"));
    }

    #[test]
    fn merge_mcp_adds_air_once_and_keeps_user_servers() {
        let v = merge_mcp(json!({"mcpServers": {"other": {"command": "x"}}}));
        assert_eq!(v["mcpServers"]["air"]["args"][0], "mcp");
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        let custom = json!({"mcpServers": {"air": {"command": "/opt/air", "args": ["mcp", "-v"]}}});
        assert_eq!(merge_mcp(custom.clone()), custom);
        assert!(merge_mcp(json!("garbage"))["mcpServers"]["air"].is_object());
    }
}
