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

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cmd::emit;

/// The roles document, embedded so installs are self-contained.
pub const ROLES_MD: &str = include_str!("../../../../docs/rules/roles.md");

/// The coordinator's procedures, embedded and installed as skills in the target repo so every
/// coordinator carries the same reasoning, versioned with `air` (owner, 2026-08-21). The
/// same text is served as MCP prompts by `air mcp`.
pub const SKILLS: &[(&str, &str)] = &[
    // air-ha8: a project adopting Air got the mechanisms (hooks, deny rules, attention
    // conditions, the removal-condition registry) and not the discipline for removing them,
    // which is the exact failure `do-less` describes. `air audit` reports what the registry
    // holds; this skill is what a reader does with it.
    //
    // `beads` was considered and deliberately left out. A target repo does need the bd
    // vocabulary, but that skill's own frontmatter says it covers "the bd 1.2.1 CLI surface",
    // and Air pins 1.2.2 because 1.2.1 corrupted the Dolt schema (`doctor::BD_PINNED`;
    // adopter adoption 2026-08-21). Installing it would ship a document describing the
    // version Air refuses — the same "surface describes something untrue" failure this bead
    // exists to fix. Add it when it is rewritten against the pinned version.
    (
        "air-do-less",
        include_str!("../../../../.claude/skills/do-less/SKILL.md"),
    ),
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

/// One change to Air's OWN surface that a repo already running Air has to be told about
/// (air-6g1).
///
/// The incident: this round moved five things under adopter, which has Air installed, and
/// nothing told it. `air install` already dry-runs; this is that dry run made honest about
/// version-to-version change.
///
/// The diff is computed against the ids recorded in `.air/installed.json`, NOT against a
/// version number. Version strings do not move on their own, and a comparison keyed to one
/// silently reports nothing the first time somebody forgets to bump it. Adding an entry here
/// is the only step: every repo installed before it then sees it.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SurfaceChange {
    /// Stable id, recorded in `.air/installed.json` once a repo has been told.
    pub id: &'static str,
    /// When it landed, and the bead, so the change is traceable to its reason.
    pub since: &'static str,
    pub headline: &'static str,
    /// True when an existing caller keeps working and is wrong, rather than erroring. These
    /// are the ones worth reading twice.
    pub silent_break: bool,
    /// What the repo does about it. "" when nothing is required.
    pub action: &'static str,
}

/// Every surface change since Air started recording them. Append; never edit an id.
pub const SURFACE: &[SurfaceChange] = &[
    SurfaceChange {
        id: "land",
        since: "2026-08-22 (air-3pz)",
        headline: "`air land [--all]` exists, and landing moved from the owner to the coordinator.",
        silent_break: false,
        action: "Alias or retire the repo's own land target; the coordinator lands, `--all` \
                 takes the queue longest-wait-first and stops at the first red.",
    },
    SurfaceChange {
        id: "close",
        since: "2026-08-22 (air-869)",
        headline: "`air close <id>… --reason` closes a landing pass in ONE bd process.",
        silent_break: false,
        action: "Use it instead of a loop over `bd close`: bd costs ~1.4 s per process here \
                 whatever it is asked, so N closes cost N × that. Denied to workers.",
    },
    SurfaceChange {
        id: "inbox-json",
        since: "2026-08-22 (air-6p5)",
        headline: "`air inbox --json` returns {captures, landings}, not a bare array.",
        silent_break: true,
        action: "Fix any script that indexes the top level as a list. It will not error: it \
                 will read zero captures and report an empty queue.",
    },
    SurfaceChange {
        id: "enforce-default",
        since: "2026-08-22 (air-i59)",
        headline: "Worker launches set AIR_ENFORCE=1: the hand-over gate refuses, it no \
                   longer advises.",
        silent_break: false,
        action: "A worker without a recorded green at HEAD is now denied the `bd` write \
                 instead of warned. Make sure the repo's verify command is the one workers \
                 actually run: `air record verify -- <cmd>`.",
    },
    SurfaceChange {
        id: "owner-label",
        since: "2026-08-22 (air-5hw)",
        headline: "The authority label is `owner`. `human` is presence only and gates nothing.",
        silent_break: true,
        action: "See the migration in docs/rules/adopting-air.md §5a. A repo that used \
                 `human` as its gate has its owner queue unfenced the moment it upgrades: \
                 exclude BOTH labels until its beads are relabelled.",
    },
    SurfaceChange {
        id: "tmux-project-names",
        since: "2026-08-22 (air-5lg)",
        headline: "tmux sessions are `<project>-<worker>`, not `<worker>`.",
        silent_break: false,
        action: "Update any `tmux attach -t <worker>` in the repo's docs or scripts.",
    },
    SurfaceChange {
        id: "bd-ms",
        since: "2026-08-22 (air-869)",
        headline: "Event lines carry bd_ms/bd_calls when the command shelled out to bd.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "land-reads-acceptance",
        since: "2026-08-22 (air-ayp)",
        headline: "`air land` closes nothing. The worker closes its own bead with proof; the \
                   landing prints each bead beside its acceptance and Air's verdict.",
        silent_break: true,
        action: "A repo whose landing pass assumed \"merged means closed\" will now find beads \
                 merged and still open, because closing moved to the worker. adopter closed \
                 99 beads on branch containment alone, 14 partial and 1 not done. The close \
                 reason is PROOF - a command and its output, a file:line, a passing test - and \
                 the hand-over gate already covers `bd close`.",
    },
    SurfaceChange {
        id: "skill-do-less",
        since: "2026-08-22 (air-ha8)",
        headline: "`air install` writes a third skill, `air-do-less`.",
        silent_break: false,
        action: "A repo running Air had the mechanisms and not the discipline for removing \
                 them. Re-run `air install --write` to get it; it is what `air audit`'s \
                 registry is read with.",
    },
];

/// What `.air/installed.json` records, so the diff has a baseline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Installed {
    /// Informational: the `air --version` that last wrote this file.
    pub air_version: String,
    pub installed_at: String,
    /// Surface ids this repo has already been told about.
    pub surface: Vec<String>,
}

/// Changes this repo has not been told about yet. Pure, so the probe does not need a repo.
pub fn surface_diff(known: &[String]) -> Vec<&'static SurfaceChange> {
    SURFACE
        .iter()
        .filter(|c| !known.iter().any(|k| k == c.id))
        .collect()
}

/// Read `.air/installed.json`. A missing or unreadable file means "told about nothing",
/// which is the right answer for a repo installed before Air recorded this.
pub fn read_installed(air_dir: &Path) -> Installed {
    std::fs::read_to_string(air_dir.join("installed.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_installed(air_dir: &Path, at: &str) -> Result<(), String> {
    let v = Installed {
        air_version: env!("CARGO_PKG_VERSION").to_string(),
        installed_at: at.to_string(),
        surface: SURFACE.iter().map(|c| c.id.to_string()).collect(),
    };
    let s = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(air_dir.join("installed.json"), s + "\n")
        .map_err(|e| format!("{}/installed.json: {e}", air_dir.display()))
}

/// The upgrade report, or "" when there is nothing to say.
pub fn render_surface(changes: &[&SurfaceChange], written: bool) -> String {
    if changes.is_empty() {
        return String::new();
    }
    let breaks = changes.iter().filter(|c| c.silent_break).count();
    let mut s = format!(
        "\nSURFACE DIFF: {} change(s) to Air since this repo was last installed",
        changes.len()
    );
    if breaks > 0 {
        s.push_str(&format!(
            ", {breaks} of which change behaviour WITHOUT erroring"
        ));
    }
    s.push('\n');
    // Marker first and fixed width: the ids vary in length, so anything after them does not
    // line up and the two that matter stop standing out.
    for c in changes {
        s.push_str(&format!(
            "  {} {}  {}\n",
            if c.silent_break { "!!" } else { "  " },
            c.id,
            c.headline
        ));
        if !c.action.is_empty() {
            s.push_str(&format!("        do: {}\n", c.action));
        }
        s.push_str(&format!("        since {}\n", c.since));
    }
    s.push_str(if written {
        "  recorded in .air/installed.json; this diff will be empty next time.\n"
    } else {
        "  read docs/rules/adopting-air.md \"Upgrading an existing installation\", then \
         `air install --write` to apply and record.\n"
    });
    s
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
    /// Air's own surface changes this repo has not been told about (air-6g1). Empty on a
    /// first install: nothing has moved under a repo that never had Air.
    surface_diff: Vec<&'static SurfaceChange>,
    previously_installed: bool,
    written: bool,
}

/// Would wiring the hooks change anything? False means Air is already installed here.
fn plan_settings_changed(before: &Value, after: &Value) -> bool {
    before != after
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

    // "Already installed" means the hooks are wired or `.air/roles.md` is there. Without
    // that, this is a first install and nothing has changed under anyone.
    let previously_installed = !plan_settings_changed(&before_settings, &after_settings)
        || air_dir.join("roles.md").exists();
    let surface_diff = if previously_installed {
        surface_diff(&read_installed(&air_dir).surface)
    } else {
        Vec::new()
    };

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
        surface_diff,
        previously_installed,
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
            // Last: the repo has now been told everything above, so record it. Written after
            // the files so a failed install does not claim the surface was delivered.
            write_installed(&air_dir, &super::now())?;
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
        s.push_str(&render_surface(&plan.surface_diff, plan.written));
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
        assert!(ROLES_MD.contains("Finishing a bead is not a stop."));
        assert!(ROLES_MD.contains("Workers are reached with `SendMessage`"));
        assert!(ROLES_MD.contains("Landing is the coordinator's: `air land --all`"));
        assert!(ROLES_MD.contains("bug `## Steps to Reproduce` + `## Acceptance Criteria`"));
        assert!(ROLES_MD.contains("epic `## Success"));
        // air-8zu: roles.md states what Air records and refuses, never one repo's closing
        // procedure. adopter closes with proof and was being told to set awaiting_review by
        // a file it cannot edit. `awaiting_review` may appear only where the refusal lists
        // what the gate matches, never as an instruction.
        assert!(
            !ROLES_MD.contains("`bd update <id> -s awaiting_review`"),
            "roles.md must not prescribe a bead-status step"
        );
        assert!(ROLES_MD.contains("is the repo's own flow, in its CLAUDE.md"));
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
        // air-ha8: do-less ships with the mechanisms, because it is the discipline for
        // removing them. `beads` deliberately does not: see the SKILLS comment.
        assert_eq!(SKILLS.len(), 3);
        assert_eq!(SKILLS[0].0, "air-do-less");
        assert!(SKILLS.iter().all(|(_, text)| text.contains("Provenance")));
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
