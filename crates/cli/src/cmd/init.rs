//! `air init [--prefix <p>] [--write]`: one command that gives a project everything Air needs
//! (plan 0005 §1; owner 2026-08-21: "a recipe that gives us everything").
//!
//! The binary cannot bundle `bd` or `claude`, so the gate comes first: both present, bd at the
//! pinned version and answering `bd list --json`, with the exact install command printed when
//! not. Then, in the target directory: `git init -b main` and a first commit if needed;
//! `.air/` in `.gitignore`; `bd init --prefix <p> --non-interactive --init-if-missing
//! --skip-agents --skip-hooks` (no AGENTS.md, no `bd prime`: its command reference conflicts
//! with Air's roles; owner 2026-08-21); `bd config set status.custom awaiting_review` (the
//! hand-over state is not a bd default);
//! `.claude/air.json` with deny patterns proposed from a scan of the repo's publish targets
//! (the adopter: a new publish target shipped outside an enumerated list; deny the verb, not the
//! tool); then `air install --write` (hooks, `.mcp.json`, roles, skills); a minimal CLAUDE.md
//! only when none exists; `air selftest`; and the next steps, which start with
//! `air record verify -- <cmd>` as the first proof (it is what made bd's corruption visible).
//! Dry run by default, like `install`.
//!
//! air-ej4 adds the empty-but-ready scaffold: the four things Air ASSUMES a repo has and used
//! to leave the adopter to discover at their first hand-over. A `Makefile` whose `verify`
//! target fails until it is edited, a `.worktreeinclude` comment header, and, inside the
//! `CLAUDE.md` stub, a pointer to `.air/roles.md` and a list of what is the repo's own (it
//! carried a hand-over sequence of its own until air-vuwx). Each follows the
//! same rule as `.claude/air.json`: created only when absent, never edited. The failing verify
//! target is the load-bearing part. A placeholder that PASSED would let the first
//! `air record verify -- make verify` record a green for a check nobody has written, and the
//! close gate reads that green.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use crate::cmd::{doctor, emit, install};

/// Deny patterns proposed from what the repo contains. Pure over a listing of paths and the
/// Makefile text; the verb is denied, never the tool.
pub fn propose_deny(
    files: &[String],
    makefile: Option<&str>,
    package_json: Option<&str>,
) -> Vec<String> {
    let has = |f: &str| {
        files
            .iter()
            .any(|p| p == f || p.ends_with(&format!("/{f}")))
    };
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        if !out.iter().any(|x| x == s) {
            out.push(s.to_string());
        }
    };
    if let Some(mk) = makefile {
        for line in mk.lines() {
            let Some((target, _)) = line.split_once(':') else {
                continue;
            };
            let t = target.trim();
            if t.is_empty() || t.contains(' ') || t.starts_with('.') || t.starts_with('#') {
                continue;
            }
            let low = t.to_ascii_lowercase();
            if ["deploy", "publish", "release", "ota", "ship"]
                .iter()
                .any(|k| low.starts_with(k))
            {
                let stem = low.split(['-', '_']).next().unwrap_or(&low).to_string();
                push(&format!("Bash(make {stem}*)"));
            }
        }
    }
    if let Some(pkg) = package_json
        && let Ok(v) = serde_json::from_str::<Value>(pkg)
        && let Some(scripts) = v.get("scripts").and_then(Value::as_object)
    {
        for k in scripts.keys() {
            let low = k.to_ascii_lowercase();
            if ["deploy", "publish", "release"]
                .iter()
                .any(|s| low.contains(s))
            {
                push(&format!("Bash(npm run {k}*)"));
                push(&format!("Bash(pnpm {k}*)"));
                push(&format!("Bash(yarn {k}*)"));
            }
        }
    }
    if has("eas.json") {
        push("Bash(eas build *)");
        push("Bash(eas submit *)");
        push("Bash(eas update *)");
    }
    if has("Fastfile") || files.iter().any(|p| p.contains("fastlane/")) {
        push("Bash(fastlane *)");
    }
    if has("fly.toml") {
        push("Bash(fly deploy *)");
        push("Bash(fly secrets *)");
    }
    if has("wrangler.toml") || has("wrangler.json") {
        push("Bash(wrangler deploy *)");
        push("Bash(wrangler pages deploy *)");
    }
    if has("Cargo.toml") {
        push("Bash(cargo publish *)");
    }
    out
}

fn run_in(dir: &Path, prog: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(prog)
        .args(args)
        .current_dir(dir)
        .env(
            "GIT_AUTHOR_NAME",
            std::env::var("GIT_AUTHOR_NAME").unwrap_or_else(|_| "air".into()),
        )
        .env(
            "GIT_AUTHOR_EMAIL",
            std::env::var("GIT_AUTHOR_EMAIL").unwrap_or_else(|_| "air@localhost".into()),
        )
        .env(
            "GIT_COMMITTER_NAME",
            std::env::var("GIT_COMMITTER_NAME").unwrap_or_else(|_| "air".into()),
        )
        .env(
            "GIT_COMMITTER_EMAIL",
            std::env::var("GIT_COMMITTER_EMAIL").unwrap_or_else(|_| "air@localhost".into()),
        )
        .output()
        .map_err(|e| format!("{prog}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{prog} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn list_files(dir: &Path) -> Vec<String> {
    let mut v = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            if p.is_dir() {
                if v.len() < 5000 {
                    stack.push(p);
                }
            } else if let Ok(rel) = p.strip_prefix(dir) {
                v.push(rel.to_string_lossy().to_string());
            }
        }
    }
    v
}

#[derive(Debug, serde::Serialize)]
struct Plan {
    dir: PathBuf,
    git: &'static str,
    claude: Option<String>,
    bd: doctor::BdCheck,
    gate_ok: bool,
    beads: &'static str,
    prefix: String,
    gitignore: &'static str,
    air_json: &'static str,
    /// What `air init` will do about Metis (air-g5o): initialise it, leave an existing
    /// workspace alone, or name the install step. Printed in the dry run, like every other
    /// step, so nothing is done that was not shown first.
    metis: String,
    proposed_deny: Vec<String>,
    claude_md: &'static str,
    scaffold: Vec<ScaffoldItem>,
    written: bool,
}

/// The CLAUDE.md `air init` writes when the repo has none. The fleet protocol is Air's and lives
/// in `.air/roles.md` (owner, 2026-09-25); this stub used to say the opposite, that the
/// hand-over flow was the repo's own, and carried a second copy of the close sequence
/// (air-vuwx). It now names only what roles.md leaves to the repo.
pub(crate) const CLAUDE_MD_STUB: &str = r#"# CLAUDE.md

This repo runs a small fleet with Air. The fleet's protocol (roles, how a bead goes from a claim
to main, proof, landing, and what Air refuses) is Air's and lives in `.air/roles.md`, appended
to every session by `air worker` / `air coordinator`. It is not restated here. Work is tracked
in beads (`bd ready`, `air claim`, `air capture`).

## What is this repo's own

- **Verify** is `make verify`, recorded as `air record verify -- make verify`. Replace the
  placeholder in the Makefile with this repo's real check.
- **Precheck**: none yet. If workers under a verification lane should run one, name it here
  and set `"precheck": true` in `.claude/air.json`.
- **Worktree setup**: untracked files a new worktree needs are listed in `.worktreeinclude`.
- **Shared resources** (a port, a simulator, Docker) are `"leases"` in `.claude/air.json`.

Domain rules for this codebase go below.
"#;

/// The header of the `Makefile` `air init` writes into a repo that has none (air-ej4).
const MAKEFILE_HEADER: &str = r#"# Written by `air init` because this repo had no Makefile. Air creates this file only when it
# is absent, and never edits it afterwards.
#
# `air record verify -- make verify` records the green that Air's one refusal reads, so this
# target is what decides whether a bead can be closed. It FAILS until you replace the
# placeholder below with this repo's own check: a verify that passes without checking anything
# records a green for an empty check, which is worse than having no verify command at all.
.PHONY: verify
verify:
"#;

/// The recipe of that target, kept separate so a probe can swap it for a real command and show
/// the failure is the placeholder rather than a broken Makefile. The `exit 1` is the point: it
/// is what stops a scaffolded repo recording a green for a check nobody has written yet.
const MAKEFILE_PLACEHOLDER: &str = "\t@echo 'placeholder: put this repo verify command in Makefile:verify, then delete this line'; exit 1\n";

/// What `air init --write` puts in a `Makefile` when the repo has none.
pub fn makefile_stub() -> String {
    format!("{MAKEFILE_HEADER}{MAKEFILE_PLACEHOLDER}")
}

/// The `.worktreeinclude` `air init` writes: a comment header and nothing else. Empty but
/// ready, because the failure it prevents is a worktree that does not build with an error that
/// does not say why.
const WORKTREEINCLUDE_STUB: &str = r#"# Files git does not track that a fresh worktree still needs in order to build: .env files,
# keys, local config. `air worker` copies everything named here out of the main checkout and
# into the new worktree. gitignore syntax, one pattern per line; `!` negations are not
# supported. Leaving this empty is fine and means a worktree gets only what git tracks.
"#;

/// One empty-but-ready file `air init --write` scaffolds (air-ej4). Created only when absent
/// and never edited, the rule `.claude/air.json` and the `CLAUDE.md` stub already follow.
#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct ScaffoldItem {
    /// Owned rather than `&'static str` since air-3xww: the journal's location is configured,
    /// so one of these paths comes from `.claude/air.json` and cannot be a literal.
    pub path: String,
    /// True when `--write` creates it. False means present, and Air leaves it alone.
    pub create: bool,
    /// What `air init` prints for this row, in dry run and after writing alike.
    pub note: &'static str,
}

/// Whether a Makefile text declares a `verify` target. Read from text somebody wrote freely,
/// so it decides only which SENTENCE prints: Air never edits an existing Makefile either way,
/// and a wrong read costs one misleading report line rather than a wrong write.
fn declares_verify(makefile: &str) -> bool {
    makefile.lines().any(|l| {
        let Some(rest) = l.strip_prefix("verify") else {
            return false;
        };
        matches!(rest.trim_start().as_bytes().first(), Some(b':'))
    })
}

/// Where each session appends what it hit (air-3xww), relative to the main checkout, when
/// `.claude/air.json` names no `journal_dir`. Inside the gitignored `.air/` since air-1qnp
/// (owner, 2026-09-25: Air's generated records do not litter the adopter's tree), and the
/// same place `handover::journal_path` resolves to from any worktree.
pub const DEFAULT_JOURNAL_DIR: &str = ".air/journal";

/// The README `air init` drops in the journal directory, so an empty directory is not a
/// mystery. Says what belongs in it and, as loudly, that nothing reads it.
const JOURNAL_README: &str = r#"# Session journal

One file per session, appended as it goes: `<session>.md`, whatever name the session has.

**What belongs here** is what the next session would want and the bead record will not carry:
a bug you hit and how it presented, a wrong turn and what corrected it, a claim you later found
was wrong, a thing you checked that turned out fine. A timestamp and a line is enough.

**The pull is toward the fix; write the state you were in instead.** By the time you write, the
fix is the part you understand best, so an entry drifts into explaining it. What the next session
needs is what you believed when you were wrong, and why it was reasonable: that is the part you
have least reason to record and they have most use for. The first worker to keep one took three
attempts to stop narrating outcomes.

**What does not**: anything already in a digest (what a bead did, and its proof), anything worth
the coordinator's inbox (`air capture`), and any work journal of what you did in order.

**Nothing gates on this.** Air does not read these files, nothing refuses without one, and no
condition counts them. It exists because a round log assembled from a coordinator's memory of
messages is lost when that coordinator hits a limit, compacts or ends — which happened the day
this was written.
"#;

/// Pure: what the scaffold would do, given what the directory already holds. `makefile` is the
/// Makefile's text when there is one; `journal` is `(dir, its README exists)`.
pub fn scaffold(
    makefile: Option<&str>,
    worktreeinclude_exists: bool,
    journal: (&str, bool),
) -> Vec<ScaffoldItem> {
    vec![
        match makefile {
            None => ScaffoldItem {
                path: "Makefile".to_string(),
                create: true,
                note: "will write a `verify` target that FAILS until you edit it",
            },
            Some(mk) if declares_verify(mk) => ScaffoldItem {
                path: "Makefile".to_string(),
                create: false,
                note: "present, declares `verify` (not touched)",
            },
            Some(_) => ScaffoldItem {
                path: "Makefile".to_string(),
                create: false,
                note: "present, NO `verify` target (not touched): add one, or \
                       `air record verify` has nothing to record and no bead can close",
            },
        },
        if worktreeinclude_exists {
            ScaffoldItem {
                path: ".worktreeinclude".to_string(),
                create: false,
                note: "present (not touched)",
            }
        } else {
            ScaffoldItem {
                path: ".worktreeinclude".to_string(),
                create: true,
                note: "will write a comment header; list the gitignored files a fresh \
                       worktree needs to build",
            }
        },
        // air-3xww. A directory with a README rather than a file: sessions create their own,
        // and an empty directory with nothing in it to say what it is for is a directory
        // nobody uses.
        if journal.1 {
            ScaffoldItem {
                path: format!("{}/README.md", journal.0),
                create: false,
                note: "present (not touched)",
            }
        } else {
            ScaffoldItem {
                path: format!("{}/README.md", journal.0),
                create: true,
                note: "will write the session journal's README; one file per session, \
                       appended, and nothing in Air reads them",
            }
        },
    ]
}

/// What `air init` will do about Metis, said before it does it (air-g5o). Three states, and
/// only one of them runs anything: an existing `.metis/` is the coordinator's plan and is never
/// re-initialised, and a missing binary is an install step named rather than a failure.
fn metis_plan(dir: &Path) -> String {
    if dir.join(".metis").is_dir() {
        return "present (not touched)".to_string();
    }
    if crate::cmd::metis::on_path() {
        return "will run `metis init <prefix>`".to_string();
    }
    "not installed; `air coordinator` will launch without it. Install from \
     https://github.com/colliery-io/metis, then re-run `air init --write`, or set \
     \"metis\": false in .claude/air.json"
        .to_string()
}

pub fn run(dir: &Path, prefix: Option<&str>, write: bool, json: bool) -> i32 {
    let dir = match dir.canonicalize() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("air init: {}: {e}", dir.display());
            return 1;
        }
    };
    // Gate: what the binary cannot install.
    let claude = Command::new("claude")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    let is_repo = run_in(&dir, "git", &["rev-parse", "--git-dir"]).is_ok();
    let has_beads = dir.join(".beads").is_dir();
    let bd = doctor::bd_check(&dir);
    // bd list only answers inside an initialised workspace; before init, presence + version
    // is the gate.
    let bd_present = bd.version.is_some();
    let gate_ok = bd_present && claude.is_some();
    let prefix = prefix.map(str::to_string).unwrap_or_else(|| {
        dir.file_name()
            .map(|s| {
                s.to_string_lossy()
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .take(4)
                    .collect()
            })
            .unwrap_or_else(|| "air".into())
    });
    let files = list_files(&dir);
    let makefile = std::fs::read_to_string(dir.join("Makefile")).ok();
    let package_json = std::fs::read_to_string(dir.join("package.json")).ok();
    let proposed_deny = propose_deny(&files, makefile.as_deref(), package_json.as_deref());
    let gitignore_has = std::fs::read_to_string(dir.join(".gitignore"))
        .map(|s| {
            s.lines()
                .any(|l| matches!(l.trim(), ".air" | ".air/" | "/.air" | "/.air/"))
        })
        .unwrap_or(false);
    let air_json_exists = dir.join(".claude/air.json").exists();
    let claude_md_exists = dir.join("CLAUDE.md").exists();
    // air-3xww: an existing `.claude/air.json` keeps whatever it names; a fresh repo gets
    // the default, and the same value is written into the config below, so the scaffold and
    // the key cannot disagree.
    let journal =
        crate::cmd::handover::journal_dir(&dir).unwrap_or_else(|| DEFAULT_JOURNAL_DIR.to_string());
    let journal_readme_exists = dir.join(&journal).join("README.md").exists();
    let scaffold = scaffold(
        makefile.as_deref(),
        dir.join(".worktreeinclude").exists(),
        (&journal, journal_readme_exists),
    );

    let mut plan = Plan {
        dir: dir.clone(),
        git: if is_repo {
            "present"
        } else {
            "will git init -b main and make a first commit"
        },
        claude: claude.clone(),
        bd,
        gate_ok,
        beads: if has_beads { "present" } else { "will bd init" },
        prefix: prefix.clone(),
        gitignore: if gitignore_has {
            "has .air/"
        } else {
            "will add .air/"
        },
        air_json: if air_json_exists {
            "present (not touched)"
        } else {
            "will write with proposed deny patterns and \"metis\": true"
        },
        metis: metis_plan(&dir),
        proposed_deny: proposed_deny.clone(),
        claude_md: if claude_md_exists {
            "present (not touched)"
        } else {
            "will write a minimal stub: the work-flow sequence and the `Bead:` trailer rule"
        },
        scaffold,
        written: false,
    };

    if !gate_ok {
        emit(json, &plan, || render(&plan));
        eprintln!(
            "air init: gate failed. {}{}",
            if !bd_present {
                "Install bd: `brew install beads && brew pin beads` (pinned 1.2.2). "
            } else {
                ""
            },
            if claude.is_none() {
                "Install Claude Code: https://code.claude.com/docs/en/setup"
            } else {
                ""
            }
        );
        return 2;
    }
    if !write {
        emit(json, &plan, || render(&plan));
        return 0;
    }
    let steps: Result<(), String> = (|| {
        if !is_repo {
            run_in(&dir, "git", &["init", "-q", "-b", "main"])?;
        }
        if run_in(&dir, "git", &["rev-parse", "HEAD"]).is_err() {
            run_in(
                &dir,
                "git",
                &["commit", "-q", "--allow-empty", "-m", "init"],
            )?;
        }
        if !gitignore_has {
            let path = dir.join(".gitignore");
            let mut s = std::fs::read_to_string(&path).unwrap_or_default();
            if !s.is_empty() && !s.ends_with('\n') {
                s.push('\n');
            }
            s.push_str(".air/\n");
            std::fs::write(&path, s).map_err(|e| format!(".gitignore: {e}"))?;
        }
        // The line is written; git has to agree (a later `!.air` or an odd layout can undo
        // it), because the ledger about to be created holds every message (air-6di).
        if let Some(why) = super::install::ignore_refusal(super::install::air_ignored(&dir)) {
            return Err(format!("refusing to continue: {why}"));
        }
        if !has_beads {
            // --skip-agents: no AGENTS.md and no `bd prime`; its command reference tells agents
            // to `bd update --claim` and `bd create`, which Air denies. --skip-hooks: no bd git
            // hooks; Air's hooks are the ones installed here.
            let bdbin = crate::cmd::claim::bd_for(&dir).bin;
            run_in(
                &dir,
                &bdbin.to_string_lossy(),
                &[
                    "init",
                    "--prefix",
                    &prefix,
                    "--non-interactive",
                    "--init-if-missing",
                    "--skip-agents",
                    "--skip-hooks",
                ],
            )?;
        }
        // The hand-over state Air's gate watches is a custom bd status; a fresh workspace
        // rejects it until declared (found dogfooding on 2026-08-22). Idempotent.
        {
            let bdbin = crate::cmd::claim::bd_for(&dir).bin;
            run_in(
                &dir,
                &bdbin.to_string_lossy(),
                &["config", "set", "status.custom", "awaiting_review"],
            )?;
        }
        if !air_json_exists {
            std::fs::create_dir_all(dir.join(".claude")).map_err(|e| format!(".claude: {e}"))?;
            // `"metis": true` by default (air-g5o, owner ruling 2026-09-06). It costs nothing
            // in a repo with no metis installed — the coordinator prints one line and
            // launches — and a default of false would mean the rule the owner asked to be
            // programmatic arrives off.
            // `"adopters": false` is written so the key EXISTS with the honest answer for a
            // fresh repo (air-jsz). It decides what `air adopter-check` does when
            // `private/adopters.md` is absent: false skips, which is right for a repo that
            // quotes nobody, and true refuses. Leaving the key out entirely is how a repo that
            // DID quote somebody ran a whole round with the check silently checking nothing.
            let v = json!({
                "worker_deny": proposed_deny,
                "coordinator_deny": [],
                "metis": true,
                "adopters": false,
                // No `journal_dir` since air-1qnp: journals default to `.air/journal/`, and
                // the key is an override for a repo that wants them tracked.
            });
            std::fs::write(
                dir.join(".claude/air.json"),
                format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default()),
            )
            .map_err(|e| format!("air.json: {e}"))?;
        }
        // Metis's own workspace, once, and only when metis can make it (air-g5o). Never
        // re-run over an existing `.metis/`: its documents are the coordinator's plan, and
        // `metis init` is not this command's to re-apply to them.
        if crate::cmd::metis::on_path() && !dir.join(".metis").is_dir() {
            run_in(&dir, "metis", &["init", &prefix])?;
        }
        if !claude_md_exists {
            std::fs::write(dir.join("CLAUDE.md"), CLAUDE_MD_STUB)
                .map_err(|e| format!("CLAUDE.md: {e}"))?;
        }
        // The empty-but-ready four (air-ej4). Two of them are the CLAUDE.md stub's own
        // paragraphs above; these two are files, and each is written ONLY when absent.
        for item in &plan.scaffold {
            if !item.create {
                continue;
            }
            let body = match item.path.as_str() {
                "Makefile" => makefile_stub(),
                ".worktreeinclude" => WORKTREEINCLUDE_STUB.to_string(),
                _ => JOURNAL_README.to_string(),
            };
            // The journal's README sits in a directory that may not exist yet; the other two
            // are at the repo root.
            if let Some(parent) = Path::new(&item.path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                std::fs::create_dir_all(dir.join(parent))
                    .map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::write(dir.join(&item.path), body)
                .map_err(|e| format!("{}: {e}", item.path))?;
        }
        Ok(())
    })();
    if let Err(e) = steps {
        eprintln!("air init: {e}");
        return 1;
    }
    // Hooks, .mcp.json, roles, skills: the install step (refuses if `air` on PATH is not us).
    let code = install::run(&dir, true, json);
    if code != 0 {
        return code;
    }
    plan.written = true;
    if !json {
        println!("{}", render(&plan));
        println!(
            "next:\n  air selftest\n  air record verify -- <your verify command>   # the first proof\n  air coordinator                                # own worktree and tmux session, channel attached\n  air worker <name> --tmux --task \"<a complete task>\""
        );
    } else {
        emit(true, &plan, String::new);
    }
    0
}

fn render(p: &Plan) -> String {
    let mut s = String::new();
    s.push_str(&format!("dir:      {}\n", p.dir.display()));
    s.push_str(&format!(
        "claude:   {}\n",
        p.claude.as_deref().unwrap_or("NOT FOUND")
    ));
    s.push_str(&format!(
        "bd:       {} (pinned {}): {}\n",
        p.bd.version.as_deref().unwrap_or("NOT FOUND"),
        p.bd.pinned,
        if p.bd.version_ok { "ok" } else { "mismatch" }
    ));
    s.push_str(&format!("git:      {}\n", p.git));
    s.push_str(&format!("beads:    {} (prefix {})\n", p.beads, p.prefix));
    s.push_str(&format!("gitignore: {}\n", p.gitignore));
    s.push_str(&format!("air.json: {}\n", p.air_json));
    s.push_str(&format!("metis: {}\n", p.metis));
    for d in &p.proposed_deny {
        s.push_str(&format!("  deny {d}\n"));
    }
    s.push_str(&format!("CLAUDE.md: {}\n", p.claude_md));
    for item in &p.scaffold {
        s.push_str(&format!("{:<9} {}\n", format!("{}:", item.path), item.note));
    }
    s.push_str(if p.written {
        "written.\n"
    } else if p.gate_ok {
        "dry run; re-run with --write to apply.\n"
    } else {
        "gate failed; see below.\n"
    });
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::{CLAUDE_MD_STUB, DEFAULT_JOURNAL_DIR, makefile_stub, propose_deny, scaffold};

    #[test]
    fn deny_patterns_name_the_verb_not_the_tool() {
        let files = vec![
            "eas.json".to_string(),
            "app/Cargo.toml".to_string(),
            "ios/fastlane/Fastfile".to_string(),
        ];
        let mk = "build:\n\tcargo build\ndeploy-api: ## ship\n\t...\ndeploy-web:\n\t...\nrelease_notes:\n\t...\n.PHONY: x\n";
        let pkg = r#"{"scripts":{"test":"jest","deploy:web":"wrangler pages deploy"}}"#;
        let d = propose_deny(&files, Some(mk), Some(pkg));
        assert!(d.contains(&"Bash(make deploy*)".to_string()), "{d:?}");
        assert_eq!(
            d.iter().filter(|x| x.contains("make deploy")).count(),
            1,
            "one pattern covers both deploy targets"
        );
        assert!(d.contains(&"Bash(make release*)".to_string()));
        assert!(d.contains(&"Bash(npm run deploy:web*)".to_string()));
        assert!(
            d.contains(&"Bash(eas build *)".to_string()) && !d.iter().any(|x| x == "Bash(eas *)")
        );
        assert!(d.contains(&"Bash(fastlane *)".to_string()));
        assert!(d.contains(&"Bash(cargo publish *)".to_string()));
        assert!(propose_deny(&[], None, None).is_empty());
    }

    #[test]
    fn the_scaffold_creates_only_what_is_absent() {
        // Fresh repo: both files are created, and the verify target Air writes fails.
        let fresh = scaffold(None, false, (DEFAULT_JOURNAL_DIR, false));
        assert!(fresh.iter().all(|i| i.create), "{fresh:?}");
        assert!(makefile_stub().contains("exit 1"), "{}", makefile_stub());

        // Present is present, whatever it holds: nothing is created either way, and the
        // Makefile with no verify target is named rather than edited.
        let with = scaffold(
            Some("verify: ## the repo's own\n\t@true\n"),
            true,
            (DEFAULT_JOURNAL_DIR, true),
        );
        assert!(with.iter().all(|i| !i.create), "{with:?}");
        assert!(
            with.iter()
                .any(|i| i.path == "Makefile" && i.note.contains("declares `verify`"))
        );
        let without = scaffold(
            Some("build:\n\t@true\nverify-scope:\n\t@true\n"),
            true,
            (DEFAULT_JOURNAL_DIR, true),
        );
        assert!(without.iter().all(|i| !i.create), "{without:?}");
        assert!(
            without
                .iter()
                .any(|i| i.path == "Makefile" && i.note.contains("NO `verify`")),
            "a verify-scope target is not a verify target: {without:?}"
        );
    }

    #[test]
    fn the_stub_points_at_roles_and_keeps_only_the_repos_own() {
        // air-vuwx: the protocol is Air's (owner, 2026-09-25), so the stub names roles.md and
        // the repo's own commands, and carries no close sequence of its own to drift from it.
        assert!(CLAUDE_MD_STUB.contains(".air/roles.md"), "{CLAUDE_MD_STUB}");
        assert!(CLAUDE_MD_STUB.contains("air record verify -- make verify"));
        for gone in ["work flow", "bd close", "git merge main"] {
            assert!(
                !CLAUDE_MD_STUB.contains(gone),
                "restates the protocol: {gone}"
            );
        }
    }
}
