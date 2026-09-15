//! Metis on the coordinator's session, and a count of the beads that name no initiative
//! (air-g5o).
//!
//! Owner, 2026-09-06, reversing the previous day's deferral: *"Metis as part of the required
//! process for the coordinator is important... It needs to be made a rule for the coordinator
//! when Air is added to a project. In fact, having it as a rule is kind of fallible, so I
//! wonder if we should make it more programmatically required. Or is that what metis does
//! basically?"*
//!
//! **No, that is not what Metis does.** Metis enforces forward-only phases on its own
//! documents; it does not enforce that anyone plans in it
//! (`docs/research/evidence.md`). So the programmatic half has to be Air's, and it
//! is exactly two things: attach it to the coordinator's session, and count what is filed
//! without it.
//!
//! **check-resources (CLAUDE.md), 2026-09-06.** Metis's MCP server and Claude Code plugin
//! already exist (colliery-io/metis, Apache-2.0): `plugins/metis/.mcp.json` declares
//! `{"command": "metis", "args": ["mcp"]}` and `plugins/metis/` is a plugin directory with its
//! own `.claude-plugin/`. The harness carries `--mcp-config` and `--plugin-dir`
//! (`docs/research/harness-facts.md`
//! 1.241), but no per-ROLE configuration: a `.mcp.json` in the repo reaches every session,
//! including the workers, and nothing in the field attaches a server to one role. That gap is
//! what this fills, and nothing more is built.
//!
//! **Not a gate.** `air status` prints how many beads declare no initiative; nothing refuses.
//! A gate comes only if the count shows the rule is ignored (owner's shape, point 4).
//!
//! Removal: the attach goes when the harness carries per-role MCP configuration natively; the
//! count goes when `bd create --validate` can require the field.

use std::path::Path;

/// Metis's own server declaration, inline (`--mcp-config` takes JSON as well as a path). Taken
/// verbatim from metis's `plugins/metis/.mcp.json`, read 2026-09-06.
///
/// Inline rather than a path, because a path is a file Air would have to write, keep in step
/// with metis's own, and explain when it drifted.
pub const MCP_CONFIG: &str = r#"{"mcpServers":{"metis":{"command":"metis","args":["mcp"]}}}"#;

/// What `.claude/air.json` says about Metis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// `"metis": true`. `air init` writes it; absent means off.
    pub on: bool,
    /// `"metis_plugin_dir": "<path>"`, when the repo declared one.
    ///
    /// **Declared, never guessed** (the `anti-brittleness` skill). Metis's plugin lives inside
    /// a checkout of metis, whose location is somebody's choice and not derivable from the
    /// `metis` binary on `PATH`. A `--plugin-dir` pointing at nothing is a silent no-op — the
    /// session comes up with no plugin and says nothing — which is the direction this must not
    /// fail in, so a path that is not a directory is reported rather than passed.
    pub plugin_dir: Option<String>,
}

/// Read `.claude/air.json` at the main checkout. Absent or unreadable means off.
pub fn config(repo: &Path) -> Config {
    let Some(v) = super::handover::air_json(repo) else {
        return Config::default();
    };
    Config {
        on: v.get("metis").and_then(serde_json::Value::as_bool) == Some(true),
        plugin_dir: v
            .get("metis_plugin_dir")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    }
}

/// Pure: what attaching Metis adds to a coordinator's argv. Empty when it is off.
///
/// `plugin_dir` is passed only when the caller has already confirmed it is a directory, so
/// this stays pure and the existence check has one home ([`plugin_dir_for`]).
pub fn argv(on: bool, plugin_dir: Option<&str>) -> Vec<String> {
    if !on {
        return Vec::new();
    }
    let mut v = vec!["--mcp-config".to_string(), MCP_CONFIG.to_string()];
    if let Some(d) = plugin_dir {
        v.push("--plugin-dir".to_string());
        v.push(d.to_string());
    }
    v
}

/// The declared plugin directory if it is one, else `None` and a line saying so.
pub fn plugin_dir_for(cfg: &Config) -> (Option<String>, Option<String>) {
    match cfg.plugin_dir.as_deref() {
        None => (
            None,
            Some(
                "air coordinator: metis is attached without its plugin (the tools, not the \
                 methodology prose). Set `metis_plugin_dir` in .claude/air.json to the \
                 `plugins/metis` directory of a metis checkout to add it."
                    .to_string(),
            ),
        ),
        Some(d) if Path::new(d).is_dir() => (Some(d.to_string()), None),
        Some(d) => (
            None,
            Some(format!(
                "air coordinator: metis_plugin_dir is not a directory: {d}. Not passed: \
                 `--plugin-dir` at a path that does not exist loads nothing and says nothing."
            )),
        ),
    }
}

/// Is `metis` runnable? A missing binary is a printed line, never a refused launch: the
/// coordinator's session is worth more than its planning tools.
pub fn on_path() -> bool {
    std::process::Command::new("metis")
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Pure: what the coordinator's launch attaches, and the lines it prints. `(argv, notes)`.
///
/// **A server that cannot start is not attached.** Handing the harness
/// `{"command": "metis"}` when `metis` is not on `PATH` buys a failed server in the session and
/// nothing else; the line below says the same thing where the owner will read it. So the
/// missing binary short-circuits, and `--plugin-dir` is only ever considered once the tool it
/// belongs to is actually there.
///
/// Nothing here refuses. A planning tool the repo asked for and does not have is worth one
/// line, never a coordinator session that will not start.
pub fn attach(cfg: &Config, on_path: bool) -> (Vec<String>, Vec<String>) {
    if !cfg.on {
        return (Vec::new(), Vec::new());
    }
    if !on_path {
        return (
            Vec::new(),
            vec![
                "air coordinator: .claude/air.json says \"metis\": true but `metis` is not on \
                 PATH, so nothing is attached. Install it \
                 (https://github.com/colliery-io/metis) or set \"metis\": false."
                    .to_string(),
            ],
        );
    }
    let (plugin_dir, note) = plugin_dir_for(cfg);
    (
        argv(true, plugin_dir.as_deref()),
        note.into_iter().collect(),
    )
}

/// The declared field a bead uses to name its initiative: a line reading `initiative: <CODE>`.
///
/// **Declared, not inferred** (`anti-brittleness`). The alternative was to look for an
/// initiative code anywhere in the description, which reads a fact out of prose somebody wrote
/// freely and fails toward COUNTING a bead as compliant because its text happened to mention
/// one. A line with the key on it is a field; a mention is not.
///
/// The value is whatever follows, trimmed, and must be one token: `initiative: ` with nothing
/// after it declares nothing.
pub fn initiative_of(description: &str) -> Option<String> {
    description.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("initiative:")?;
        let v = rest.trim();
        (!v.is_empty() && !v.contains(char::is_whitespace)).then(|| v.to_string())
    })
}

/// How many of these beads declare no initiative, and how many were looked at.
///
/// Epics are excluded: an epic is a container filed FROM an initiative's decompose phase, and
/// counting one would double-count its children's provenance.
pub fn without_initiative(issues: &[air_bd::Issue]) -> (usize, usize) {
    let considered: Vec<&air_bd::Issue> = issues
        .iter()
        .filter(|i| i.issue_type != super::ready_cache::EPIC)
        .collect();
    let missing = considered
        .iter()
        .filter(|i| {
            initiative_of(&i.description).is_none()
                && initiative_of(&i.acceptance_criteria).is_none()
        })
        .count();
    (missing, considered.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn argv_is_empty_when_off_and_carries_both_when_declared() {
        assert!(argv(false, Some("/p")).is_empty());
        assert_eq!(argv(true, None), vec!["--mcp-config", MCP_CONFIG]);
        let both = argv(true, Some("/p"));
        assert_eq!(both[2], "--plugin-dir");
        assert_eq!(both[3], "/p");
    }

    #[test]
    fn a_plugin_dir_that_is_not_a_directory_is_reported_not_passed() {
        let (d, note) = plugin_dir_for(&Config {
            on: true,
            plugin_dir: Some("/nonexistent-zz/plugins/metis".into()),
        });
        assert!(d.is_none());
        assert!(note.unwrap().contains("not a directory"));
        let tmp = tempfile::tempdir().unwrap();
        let (d, note) = plugin_dir_for(&Config {
            on: true,
            plugin_dir: Some(tmp.path().to_string_lossy().to_string()),
        });
        assert!(d.is_some() && note.is_none());
    }

    #[test]
    fn an_initiative_is_a_declared_line_not_a_mention() {
        assert_eq!(
            initiative_of("blah\ninitiative: PLAT-3\nmore"),
            Some("PLAT-3".to_string())
        );
        assert_eq!(initiative_of("  initiative:PLAT-3"), Some("PLAT-3".into()));
        // A mention is not a declaration.
        assert_eq!(initiative_of("part of the PLAT-3 initiative"), None);
        // A key with nothing after it declares nothing.
        assert_eq!(initiative_of("initiative:"), None);
        assert_eq!(initiative_of("initiative:   "), None);
        // Nor does a sentence that begins with the key.
        assert_eq!(initiative_of("initiative: the one we agreed"), None);
    }

    #[test]
    fn epics_are_not_counted_and_either_field_declares() {
        let issue = |id: &str, ty: &str, desc: &str| air_bd::Issue {
            id: id.into(),
            issue_type: ty.into(),
            description: desc.into(),
            ..Default::default()
        };
        let mut with_field = issue("d", "task", "");
        with_field.acceptance_criteria = "initiative: PLAT-9".into();
        let all = vec![
            issue("a", "task", "initiative: PLAT-1"),
            issue("b", "task", "no field here"),
            issue("c", "epic", "no field here either"),
            with_field,
        ];
        assert_eq!(without_initiative(&all), (1, 3));
    }
}
