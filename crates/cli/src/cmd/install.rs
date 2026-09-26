//! `air install [--write]`: wire Air into a target repo's Claude Code config.
//!
//! What it touches, and only with `--write` (dry run prints the exact before/after):
//! - `<repo>/.claude/settings.json`: the hook entries, merged (never duplicated, never
//!   removing anything that is not ours).
//! - `<repo>/.mcp.json`: the `air` server (`air mcp`), merged the same way.
//! - `<repo>/.air/`: created; `roles.md` written from the copy embedded in this binary so the
//!   launchers can pass it with `--append-system-prompt-file`.
//! - `.gitignore`: `--write` REFUSES when `git check-ignore -q .air` fails (air-6di: since
//!   air-srv the ledger holds the text of every agent-to-agent message, and a stranger's
//!   first `git add -A` is the recorded shape of the failure); it names the fix and does not
//!   edit the file. Removed when the ledger no longer holds content a person would call
//!   private, or when `air init`'s own ignore line leaves the check nothing to refuse.
//!
//! It never touches the live fleet's state beyond these files, and it refuses to write when
//! the `air` on PATH is not this binary (enforcement rank 10: a worktree copy must not be
//! what the hooks resolve to). `--pin` lifts that refusal because it removes its cause: the
//! binary is copied to `.air/bin/air` and the hooks and channel name the copy by absolute
//! path, so what they run is this binary whatever PATH says (air-4usc). A pinned repo refuses
//! a plain `--write` for the same reason, until `--pin` or `--unpin` says which binary wins.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cmd::emit;

/// The roles document, embedded so installs are self-contained.
pub const ROLES_MD: &str = include_str!("../../../../docs/rules/roles.md");

/// Appended to the COORDINATOR's role prose when Metis is attached (air-g5o), and to no other
/// session.
///
/// Metis's own plugin text declares Metis the system of record and says plans do not live
/// outside it. That is true of a repo that runs Metis alone and false here, where tasks are
/// beads and decisions are dated files in `docs/`. A plugin's instructions arrive in the
/// session whether or not anyone agreed with them, so the boundary is stated on Air's side of
/// the prompt rather than argued with afterwards (analysis 2026-09-05, air-ate).
///
/// Pinned by `install::tests::the_metis_split_states_the_boundary`, because the failure it
/// prevents is a paragraph quietly going missing and the coordinator taking a tool's word for
/// where the plan lives.
pub const METIS_SPLIT: &str = "\
## Metis, and what it is not the system of record for

Metis is attached to this session for **vision and initiatives**: the long-lived shape of the
work, and the phases an initiative moves through. Plan there, and cut work from an initiative
at its decompose phase.

It is not the task tracker and it is not the decision log. **Tasks are beads** — filed with
`bd create --validate`, claimed with `air claim`, closed with proof — and each one's
description declares the initiative it came from on a line reading `initiative: <CODE>`.
**Decisions are dated entries in `docs/`.** Metis's own instructions say otherwise, because
they are written for a repo that runs Metis alone; this is not one.

`air status` prints how many open beads declare no initiative. It is a count, not a gate:
nothing is refused for lacking one.
";

/// The coordinator's procedures, embedded and installed as skills in the target repo so every
/// coordinator carries the same reasoning, versioned with `air` (owner, 2026-08-21).
///
/// This used to end "the same text is served as MCP prompts by `air mcp`". It never was:
/// `prompts/list` returns `[]` and `mcp.rs:796` asserts it (air-w0e). A surface describing
/// something untrue is air-ha8's defect, and a doc comment is a surface.
pub const SKILLS: &[(&str, &str)] = &[
    // air-ha8: a project adopting Air got the mechanisms (hooks, deny rules, attention
    // conditions, the removal-condition registry) and not the discipline for removing them,
    // which is the exact failure `do-less` describes. `air audit` reports what the registry
    // holds; this skill is what a reader does with it.
    //
    // `beads` was considered and deliberately left out. A target repo does need the bd
    // vocabulary, but that skill's own frontmatter says it covers "the bd 1.2.1 CLI surface",
    // and Air pins 1.2.2 because 1.2.1 corrupted the Dolt schema (`doctor::BD_PINNED`;
    // The adopter's adoption 2026-08-21). Installing it would ship a document describing the
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
    //
    // `phase-transitions` was installed too, until 2026-09-25 (air-vuwx): its bead state
    // machine (`awaiting_review`, close only after landing, a re-verifying `air land`) was a
    // second fleet protocol beside `.air/roles.md`, and the owner ruled that day that the
    // protocol is Air's and lives in roles.md. With the bead half pointing there, what is left
    // is Metis's epic vocabulary, which `decomposition` already carries. Re-add it only if an
    // adopter's coordinator is seen needing epic states `decomposition` does not give.
    //
    // The sources stay in this repo's `.claude/skills/`, so this repo carries `do-less` and
    // `air-do-less` side by side. Moving them would rename the skill CLAUDE.md invokes by name
    // and part each SKILL.md from the `references/` it cites; the price of staying is one
    // extra line per skill in a session's skill listing. Move them if the duplicate is ever
    // seen invoked in place of the source.
];

/// Skills `air install` once wrote and no longer ships. `--write` removes each one's directory
/// and reports it; nothing else under `.claude/skills/` is touched. Append a name when a skill
/// leaves [`SKILLS`]; never remove one, because a repo installed long ago may still carry it.
///
/// Before this, a retired skill stayed installed until an adopter deleted it by hand, which a
/// notice asked them to do (an adopter's re-audit, 2026-09-25, air-rr98). Air owns the `air-`
/// names, so removing them takes nothing that is the repo's.
pub const RETIRED_SKILLS: &[&str] = &["air-phase-transitions"];

/// Pure: the retired skills present in a repo, given which `air-*` directories exist there.
pub fn retired_present(existing: &[String]) -> Vec<String> {
    RETIRED_SKILLS
        .iter()
        .filter(|r| existing.iter().any(|e| e == *r))
        .map(|r| (*r).to_string())
        .collect()
}

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

/// The wall-clock cap written into `settings.json` for `air hook`.
///
/// **Fail direction: OPEN, and silently.** Claude Code KILLS the hook at this cap; a killed
/// process writes no event line, so a gate that never ran is indistinguishable in the record
/// from a gate that allowed. Every other budget here fails toward a message; this one fails
/// toward nothing at all.
///
/// **Not derived, and it cannot be from its own distribution.** A budget whose overrun deletes
/// its own sample is censored at exactly the value you would want to size against, so
/// `air audit` pairs the `hook` percentiles with an unpaired-hook count: the PreToolUse
/// invocations with no PostToolUse to match them. That count, not the p99, is what moves this
/// number — a non-zero one means hooks are being killed and the cap is short. The design
/// budget is p99 ≤ 150 ms (tick 0315), so 5 s is ~33x it; raising the cap raises the price of
/// every wedged hook the fleet waits on, which is why it is not simply generous.
///
/// `hook::HOOK_BUDGET` mirrors this so the process measures itself against the number
/// actually installed.
pub const HOOK_TIMEOUT_SECS: u64 = 5;

/// The hook table (plan 0001 §5): one command for every event, short timeout.
pub fn hook_entries() -> Vec<(&'static str, Option<&'static str>)> {
    vec![
        ("SessionStart", None),
        // `SendMessage` is here to be COUNTED, not gated (air-q07). The cost the owner most
        // wants minimised — agent-to-agent coordination — was the one thing the ledger did
        // not contain: one worker sent ~46,900 characters in a day and a query over the event
        // log returned zero, not because there was none but because it was invisible.
        // `AskUserQuestion` is here to be COUNTED (air-bm3): the tool is denied to workers,
        // and without the event the deny's removal condition could never be settled.
        (
            "PreToolUse",
            Some("Edit|Write|MultiEdit|Bash|SendMessage|AskUserQuestion"),
        ),
        ("PostToolUse", Some("Edit|Write|MultiEdit|Bash")),
        ("PermissionRequest", None),
        ("PermissionDenied", None),
        ("PostToolUseFailure", None),
        ("Stop", None),
        ("SubagentStop", None),
        // air-1n3: no hook fires at a usage limit or at its reset, and these two are the
        // nearest the harness has. `Notification` carries the `quota_auto_resume_*` types,
        // which are the only first-party word about whether the harness is bringing a session
        // back; `StopFailure` fires when a turn ends on an API error. Both are RECORDED and
        // neither is gated. Unmatched on purpose: a matcher would have to enumerate
        // notification types, and the one Air most needs to see is the one it has not met yet.
        ("Notification", None),
        ("StopFailure", None),
        ("SessionEnd", None),
    ]
}

fn air_hook(command: &str) -> Value {
    json!({"type": "command", "command": command, "timeout": HOOK_TIMEOUT_SECS})
}

fn is_ours(h: &Value) -> bool {
    h.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c == "air hook" || c.ends_with("/air hook") || c.ends_with("/air' hook"))
}

/// Where `air install --write --pin` puts its copy of the binary (air-4usc): `.air/bin/air`,
/// so it is ignored with the rest of `.air/`.
///
/// Why: the owner builds Air with Air (2026-09-25), and a candidate build must run in a repo
/// without replacing the `air` on PATH that the fleet building it depends on. A copy, not a
/// path into `target/`: CLAUDE.md's rule that a repo's tooling never depends on Air's build
/// stays true. Removed when Air is no longer built by a fleet that runs Air.
pub fn pin_path(air_dir: &Path) -> PathBuf {
    air_dir.join("bin").join("air")
}

/// The hook command: the pinned copy by absolute path, shell-quoted because the harness runs
/// it through a shell, or `air hook` from PATH.
pub fn hook_command(pin: Option<&Path>) -> String {
    match pin {
        Some(p) => format!(
            "{} hook",
            crate::cmd::launch::shell_quote(&p.display().to_string())
        ),
        None => "air hook".to_string(),
    }
}

/// Pure: merge our hook entries into a settings object with `air hook` from PATH. Idempotent.
pub fn merge_hooks(settings: Value) -> Value {
    merge_hooks_with(settings, "air hook")
}

/// Does git ignore the ledger in this repo? `git check-ignore -q .air/ledger.db` is the same
/// answer git gives `git add -A`, wherever the rule lives. The file, not the directory: before
/// the first install `.air` does not exist, and a directory-only pattern (`.air/`, the one
/// every doc recommends) cannot match a path git cannot see as a directory, while a path
/// inside it matches either way. False outside a git repo.
pub fn air_ignored(repo: &Path) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["check-ignore", "-q", ".air/ledger.db"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Pure: the refusal `--write` prints when `.air` is not ignored, with the fix (air-6di).
/// `None` when it is. No bypass flag: the ledger holds every agent-to-agent message since
/// air-srv, and the failure this prevents is one `git add -A`.
pub fn ignore_refusal(ignored: bool) -> Option<String> {
    (!ignored).then(|| {
        "`.air/` is not ignored (`git check-ignore -q .air` fails) and the ledger in it holds \
         the text of every agent-to-agent message. Fix: echo '.air/' >> .gitignore"
            .to_string()
    })
}

/// Pure: merge our hook entries into a settings object, each running `command`. Idempotent,
/// and an existing Air hook whose command differs is rewritten, which is how `--pin` and
/// `--unpin` move a repo between the pinned copy and PATH (air-4usc).
pub fn merge_hooks_with(mut settings: Value, command: &str) -> Value {
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
        // Ours already there? Then the only thing that can be stale is the matcher, and it
        // must be UPDATED rather than left (air-q07). Before this, `merge_hooks` treated
        // "an air hook exists for this event" as done, so a repo that installed Air once kept
        // its first matcher for ever and re-running `air install --write` changed nothing —
        // which is how widening the matcher to `SendMessage` would have reported success and
        // recorded no messages. The idempotence that matters is "running it twice is the same
        // as running it once", not "never touch what is there".
        let ours = arr.iter_mut().find(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_some_and(|hs| hs.iter().any(is_ours))
        });
        if let Some(group) = ours {
            for h in group
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .into_iter()
                .flatten()
                .filter(|h| is_ours(h))
            {
                if h.get("command").and_then(Value::as_str) != Some(command)
                    && let Some(obj) = h.as_object_mut()
                {
                    obj.insert("command".into(), json!(command));
                }
            }
            match (matcher, group.get("matcher").and_then(Value::as_str)) {
                (Some(want), have) if have != Some(want) => {
                    if let Some(obj) = group.as_object_mut() {
                        obj.insert("matcher".into(), json!(want));
                    }
                }
                _ => {}
            }
            continue;
        }
        let group = match matcher {
            Some(m) => json!({"matcher": m, "hooks": [air_hook(command)]}),
            None => json!({"hooks": [air_hook(command)]}),
        };
        arr.push(group);
    }
    settings
}

/// Pure: merge the `air` MCP server into a `.mcp.json` object, running `command` (`air`, or
/// the pin). Idempotent. An entry Air wrote (`air`, or a pin under `.air/bin/`) is pointed at
/// `command`; one somebody wrote by hand is left alone.
pub fn merge_mcp_with(mut mcp: Value, command: &str) -> Value {
    if !mcp.is_object() {
        mcp = json!({});
    }
    if let Some(servers) = mcp
        .as_object_mut()
        .map(|o| o.entry("mcpServers").or_insert_with(|| json!({})))
        .and_then(Value::as_object_mut)
    {
        let entry = servers
            .entry("air")
            .or_insert_with(|| json!({"command": command, "args": ["mcp"]}));
        let ours = entry
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|c| c == "air" || c.ends_with("/.air/bin/air"));
        if ours && let Some(obj) = entry.as_object_mut() {
            obj.insert("command".into(), json!(command));
        }
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
/// The incident: this round moved five things under the adopter, which has Air installed, and
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

/// Every surface change since Air started recording them. Append; never edit an id. A lane
/// appends the notice only; the release row that covers it is appended by the coordinator at
/// round end (air-mir), and `make release` refuses until the two agree.
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
                 merged and still open, because closing moved to the worker. The adopter closed \
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
    // ---- 2026-08-29 ----------------------------------------------------------------
    // Derived from the day's 50 landings on main, not from a list written afterwards
    // (air-njb). The filter is the acceptance's: a change that alters what a target repo
    // SEES, DOES, or MAY DO. Seventeen of the fifty touch `install.rs`, `launch.rs`,
    // `schema.rs`, `main.rs` or `roles.md`; the rest are internal and are not here.
    SurfaceChange {
        // First, because it is about the installer itself and it changes what every other
        // notice in this list is worth.
        id: "install-refreshes-matcher",
        since: "2026-08-29 (air-q07)",
        headline: "`air install --write` now UPDATES a stale hook matcher. Until today it \
                   skipped any event that already had an `air hook` entry, so a re-run never \
                   changed a matcher and your repo still has the one it was FIRST installed \
                   with.",
        silent_break: true,
        action: "Check, do not assume: `grep -o '\"matcher\": \"[^\"]*\"' .claude/settings.json`. \
                 If a matcher there is narrower than the one this version installs, it has \
                 been narrower since your first install and every widening since was a \
                 no-op that reported success. Re-run `air install --write` and diff the file. \
                 The trap underneath - that the installer is only as new as the binary, so an \
                 old `air` cannot write a new matcher however many times you run it - is now \
                 CHECKED rather than left to you: since air-w9d `air install` refuses to write \
                 when this repo was last installed by a newer air, and names both builds. You \
                 no longer have to compare versions by eye.",
    },
    SurfaceChange {
        id: "sendmessage-measured",
        since: "2026-08-29 (air-q07)",
        headline: "The PreToolUse matcher includes `SendMessage`: agent-to-agent messages are \
                   counted (sender, recipient, byte count -- never content).",
        silent_break: true,
        action: "Re-run `air install --write`. Until you do, `air audit`'s traffic block reads \
                 zero for this repo, and a zero there is indistinguishable from silence -- \
                 which is the bug it replaced: one worker sent ~46,900 characters in a day and \
                 the event log contained none of it. Nothing is gated; there is no threshold \
                 and no refusal.",
    },
    SurfaceChange {
        id: "coordinator-may-commit",
        since: "2026-08-29 (air-iy1)",
        headline: "`Bash(git commit *)` is out of COORDINATOR_DENY. The boundary is the \
                   remote, not main: `git push` is still denied.",
        silent_break: false,
        action: "Your coordinator GAINS a permission on its next launch - it can commit on \
                 main by hand, which it could not before. Nothing forces it to; `air land` is \
                 still how a worker's branch reaches main. If your repo relied on the deny to \
                 keep main untouched by the coordinator, that is no longer what stops it.",
    },
    SurfaceChange {
        id: "ledger-v12",
        since: "2026-08-29 (air-air, air-4cr)",
        headline: "Ledger schema 11 -> 12: `sessions.model`, and a `verify_inflight` table \
                   behind `air status`'s in-flight verify reporting.",
        silent_break: false,
        action: "It migrates forward on the first write by the new binary and needs nothing \
                 from you. It does NOT migrate backwards: if two `air` binaries of different \
                 ages share one `.air/ledger.db`, upgrade them together. That is the case \
                 worth checking before the first run, not after.",
    },
    SurfaceChange {
        id: "land-fast-forward",
        since: "2026-08-29 (air-odv)",
        headline: "`air land` no longer merges, verifies on main, and rewinds on red. It builds \
                   the landing commit off main with `git commit-tree` and fast-forwards onto \
                   it, and it RUNS NO VERIFY: the branch must contain main, so that commit's \
                   tree is byte-identical to the one the worker already recorded a green for. \
                   No armed window, no rollback, and the dirty-main refusal went with the \
                   `git reset --hard` that was its only reason.",
        silent_break: false,
        action: "If you have your own lander, this is FYI and not an instruction to adopt \
                 ours: the point is that Air's changed shape, so a script that assumed `air \
                 land` could leave main mid-merge, that a verify runs during a landing, or \
                 that a dirty-tree refusal would stop one, is reasoning about behaviour that \
                 is gone. If you do use `air land`, nothing changes at the call site - but \
                 note the safety now rests entirely on the two preconditions it already \
                 enforced (branch contains main, recorded green at the branch head), so do \
                 not relax either.",
    },
    SurfaceChange {
        id: "review-waiting-deleted",
        since: "2026-08-29 (air-okc)",
        headline: "`review-waiting` and its whole support chain are deleted: the condition, \
                   `air status`'s `review: N waiting` line, and the `bd list --status \
                   awaiting_review` call that fed them.",
        silent_break: true,
        action: "It last fired here on 2026-08-22 because close-with-proof replaced hand-over \
                 (air-7o3) and the state stopped occurring. **If your repo still uses \
                 `awaiting_review`, you are the case this was deleted without**: a surface you \
                 may be reading is simply gone, and a script grepping `air status` for \
                 \"review:\" now finds nothing rather than 0. Say so and it can come back - it \
                 was removed on this repo's evidence, not yours.",
    },
    SurfaceChange {
        id: "audit-splits-pushes",
        since: "2026-08-29 (air-5uz)",
        headline: "`air audit` no longer reports \"fires\". A condition now has separate \
                   `evaluated` and `pushed` counts, because the poll re-evaluates and calling \
                   that a firing read 10,722 log lines as 45 real pushes.",
        silent_break: true,
        action: "A script reading the old field gets nothing and reports zero rather than \
                 erroring. Also read the counts differently than before: any `pushed` figure \
                 from a day before 2026-08-29 is uninformative, because nothing recorded a \
                 push until this landed. A zero there is a fact about what was countable.",
    },
    SurfaceChange {
        id: "gc-retention",
        since: "2026-08-29 (air-i7s)",
        headline: "`air gc [--keep-days N] [--apply]` exists and DELETES event files under \
                   `.air/events/`. It is manual: nothing runs it for you.",
        silent_break: false,
        action: "If you archive or ship `.air/events/` anywhere, know that a command now \
                 removes files from it. Dry-run first - without `--apply` it only reports.",
    },
    SurfaceChange {
        id: "triage-one-at-a-time",
        since: "2026-08-29 (air-zlq)",
        headline: "`air triage` takes exactly ONE capture id. Batch mode is deleted.",
        silent_break: false,
        action: "A caller passing several ids now fails loudly instead of silently handling \
                 about two of them under one budget. Loop instead.",
    },
    SurfaceChange {
        id: "fence-deleted",
        since: "2026-08-29 (air-9u6)",
        headline: "The cross-project fence is deleted. Air no longer denies a `tmux` command \
                   naming another project's session.",
        silent_break: true,
        action: "It never fired once in any recorded day, which is why it went. If you were \
                 counting on it as isolation between fleets on one machine, you were counting \
                 on something that had never acted - the rule stands in `roles.md`, the \
                 refusal does not.",
    },
    SurfaceChange {
        id: "selftest-prove",
        since: "2026-08-29 (air-682)",
        headline: "`air selftest --prove` runs each probe's declared mutation. It EDITS \
                   TRACKED FILES and rebuilds, and refuses to start on a dirty tree.",
        silent_break: false,
        action: "Do not put it in CI beside `air selftest`, and do not run it over \
                 uncommitted work. Plain `air selftest` is unchanged and still what `make \
                 verify` should call.",
    },
    SurfaceChange {
        id: "dated-cutoffs",
        since: "2026-08-29 (air-24e)",
        headline: "`air doctor` reports every rule with a date in it and whether that date has \
                   passed. Both of Air's expired on 2026-08-23 and made this repo's test \
                   suite red for six days with nothing failing at the moment it broke.",
        silent_break: true,
        action: "Run `air doctor` and read the `dated rule` lines. If yours say EXPIRED, the \
                 fallback is dead: a commit now needs a `Bead:` trailer to be attributed and a \
                 digest needs a `bead:` front-matter line to satisfy the gate. Fixtures in \
                 your own tests that write a commit or a digest \"now\" were inside the window \
                 when they were written and are outside it now - that is what went red here, \
                 and the same clock is in your copy.",
    },
    SurfaceChange {
        id: "messages-table",
        since: "2026-09-05 (air-srv)",
        headline: "`.air/ledger.db` now holds the CONTENT of every `SendMessage` a session \
                   sends, in a `messages` table (schema v13). The event line is unchanged: \
                   recipient and byte count, never the text.",
        silent_break: false,
        action: "Nothing to run; the table is created on the next hook. Know that the ledger \
                 file is now a transcript of agent-to-agent traffic: `sqlite3 .air/ledger.db \
                 'select * from messages'` reads it, `air doctor` counts it. Only the send side \
                 is a tool call, so a message from another fleet is in that fleet's ledger.",
    },
    SurfaceChange {
        id: "verify-key",
        since: "2026-09-05 (air-7wf)",
        headline: "A recorded green is keyed by COMMIT, by whichever worker ran it; the worker \
                   is no longer part of the gate's key. Opt in to keying by TREE with \
                   `\"verify_key\": \"tree\"` in .claude/air.json, and `air status` names a \
                   tree green either way instead of reading `not green` after every landing.",
        silent_break: true,
        action: "Two things changed under you. (1) Any worker's green at a sha now satisfies \
                 every worker's gate at that sha: a batching lane's one verify stands for the \
                 workers that fast-forward onto it. Who ran it is still on the row. (2) The \
                 tree key is NOT on by default and must not be turned on by reflex: a green \
                 transfers to an identical tree only if your verify is a function of the tree \
                 alone. The adopter's is not - `make verify` runs `git log main..HEAD` to pick \
                 the beads it checks (scripts/lib/bead_citations.py:140), so two commits over \
                 one tree verify differently there and a tree-keyed gate would pass beads it \
                 never checked. Ten-minute check before declaring it: grep your verify for \
                 `git log`, `rev-list`, `describe` and anything reading commit messages. Without \
                 the declaration nothing about the gate's strictness changed; the price is one \
                 re-verify per landed bead, which `air status` now names as such.",
    },
    SurfaceChange {
        id: "killed-is-no-verdict",
        since: "2026-09-05 (air-ppm)",
        headline: "A verify that exits 143 or 137 is recorded as KILLED, not red: it is no \
                   verdict for that sha. A child that died by signal is recorded as 128+signal \
                   rather than -1.",
        silent_break: true,
        action: "A run your harness killed at its timeout no longer reads as a red at HEAD, \
                 no longer makes a green/kill pair read as flaky, and no longer tells the \
                 worker \"the repo's test is the bug\". The exit code is still recorded and \
                 `air record` still mirrors it, so a script gated on its exit sees what it \
                 saw. Only 137 and 143 are read this way: when `make`'s CHILD is the process \
                 signalled, make exits 2 and Air records a red, because 2 is a real failure's \
                 code too and Air does not parse make's \"Terminated\" line to tell them \
                 apart. A wrapper that knows a stage was signalled should exit 143 to say so \
                 (the adopter's run-logged.sh does).",
    },
    SurfaceChange {
        id: "task-by-file",
        since: "2026-09-05 (air-er0)",
        headline: "`air worker --task` writes the task to `<main>/.air/tasks/<name>.md`; the \
                   process's command line carries a fixed sentence naming that path, never \
                   the task text.",
        silent_break: false,
        action: "Nothing to change in how workers are launched. What changes is what `ps` \
                 shows: a worker's argv no longer contains its prompt, so a `pkill -f` over \
                 ordinary command text (`air record verify`, `git status`) stops matching \
                 every peer. The adopter lost seven workers to that on 2026-08-30. Anything \
                 that reads a worker's task out of `ps` or the tmux command line reads the \
                 file instead; anything that cleans `.air/` leaves `tasks/` alone.",
    },
    SurfaceChange {
        id: "land-worker",
        since: "2026-09-05 (air-09b)",
        headline: "`air land --worker <name>` names the branch to land. `air land <bead>` is \
                   refused when more than one branch carries the bead, naming each carrier; \
                   `air status` and `air inbox --owner` now offer the `--worker` form.",
        silent_break: false,
        action: "Land by branch: `air land --worker <name>`, which merges that branch with \
                 every bead its range names. Naming a bead still works while exactly one \
                 branch carries it. A script that greps the offered command for `air land \
                 <bead>` reads `air land --worker <name>` now.",
    },
    SurfaceChange {
        id: "owner-inbox-gone",
        since: "2026-09-05 (air-uef)",
        headline: "The owner inbox is gone: `air capture --for owner` is refused, `air inbox \
                   --owner` and the `owner-decision-waiting` condition no longer exist. The \
                   owner's queue is beads labelled `owner`, counted on the `ready:` line of \
                   `air status`.",
        silent_break: true,
        action: "A rule or script that runs `air capture --for owner` now gets a refusal \
                 (exit 2) naming the replacement; one that runs `air inbox --owner` gets a \
                 clap error. Captures an older binary wrote for the owner are still there: \
                 `air inbox` lists every open capture, so triage them on the next pass. The \
                 coordinator triages EVERY capture into a bead or drops it with a reason, and \
                 labels the bead `owner` (with its recommendation in the description) when the \
                 decision is the owner's. `air://owner-queue` is gone from `air mcp`.",
    },
    SurfaceChange {
        id: "land-refuses-in-flight",
        since: "2026-09-05 (air-1bm)",
        headline: "`air land` REFUSES while any verify is in flight, naming each run and its \
                   pid. It used to warn and land anyway. `--despite-inflight` lands regardless \
                   and is recorded on the landings row (`despite_inflight`, schema v16) and the \
                   event line.",
        silent_break: false,
        action: "A landing that used to print a warning and proceed now exits 2 with the runs \
                 named. Wait (`air status` shows when they exit), stop one by pid (`kill \
                 <pid>`, never `pkill -f`), or pass `--despite-inflight` knowing it destroys \
                 those runs. The adopter lost 1,199 s of finished verify to the warning on \
                 2026-08-30 and an operational rule did not hold; the override count is what \
                 decides whether the refusal stays.",
    },
    SurfaceChange {
        id: "handover-names-the-bead",
        since: "2026-09-05 (air-xbl)",
        headline: "`air handover` from a worktree holding no claim (and naming no bead) no \
                   longer fails the digest check unconditionally: with nothing to declare, \
                   the check is skipped. A refusal names the bead the worker holds instead \
                   of a literal `<bead>`.",
        silent_break: false,
        action: "Nothing to run. A batching lane that merges other workers' green work and \
                 holds no claim can now hand over; a worker that holds a claim or names a \
                 bead must still declare it in the digest's `bead:` front matter, unchanged. \
                 If a rule of yours told workers to expect `<bead>` in the message, delete it.",
    },
    SurfaceChange {
        id: "env-on-the-process",
        since: "2026-09-05 (air-9dg)",
        headline: "AIR_ENFORCE, AIR_ROLE, AIR_PROJECT and BEADS_ACTOR are set on the spawned \
                   process (and by `tmux new-session -e` on the detached path); a pass-through \
                   `--settings` is MERGED into Air's, and `air status` marks a worker whose \
                   hooks do not see AIR_ENFORCE=1 as UNENFORCED (ledger v15).",
        silent_break: true,
        action: "Check every launch of yours for a second `--settings`: `air worker … -- \
                 --settings '{…}'` used to REPLACE Air's env block and switch the one refusal \
                 off with no message (the adopter ran five hours unenforced after adding one to \
                 disable Remote Control, and found out from a close that should have been \
                 refused). Inline JSON now merges; a `--settings <file>` in pass-through is \
                 refused with the four names. Relaunch every worker through `air worker` and \
                 read `air status`: an UNENFORCED line is a session whose hooks still run \
                 without the env. The detached path needs tmux >= 3.2 for `-e`.",
    },
    SurfaceChange {
        id: "handover-carried-bead",
        since: "2026-09-05 (air-60x)",
        headline: "`air handover <bead>` accepts a bead the branch CARRIES by a `Bead:` \
                   trailer in main..HEAD, not only one the worker has claimed. A branch that \
                   supersedes another worker's closed bead now has a hand-over path, and the \
                   gate agrees with `air land` on what makes a branch handable.",
        silent_break: false,
        action: "Nothing to run. The claim refusal's fix is now the trailer (`Bead: <id>` on \
                 the commit that does the work), never `air claim <id>`: the bead may be \
                 closed and another worker's. The Stop advisory also speaks to a worker whose \
                 branch carries a bead by trailer, not only to one holding a claim. Delete any \
                 rule of yours that said a superseding branch must be handed over by hand.",
    },
    SurfaceChange {
        id: "ready-split-epics",
        since: "2026-09-05 (air-f10)",
        headline: "The claimable count excludes epics and the `ready:` line names them apart \
                   (`N epic(s) to decompose, not claimable`); `air claim <epic>` is refused. \
                   The Stop nudge and `idle-without-claim` read the same split.",
        silent_break: true,
        action: "Your `ready:` line's claimable number may drop, and that is the true count: \
                 `2 claimable` read as two workers' worth of work when both were containers. \
                 A rule or script that claims an epic to \"own\" it now gets a refusal; \
                 decompose it with `bd create` children instead. Nothing else to run.",
    },
    SurfaceChange {
        id: "bd-calls-per-event",
        since: "2026-09-05 (air-bp0)",
        headline: "`bd_calls`/`bd_ms` on an event line are that event's own cost. Lines from \
                   `air mcp` (`status.attention`, `channel.push`) used to carry the server's \
                   running lifetime total, restamped on every tick. `air status` looks up \
                   every reconciled claim in one `bd show`, and SubagentStop no longer runs \
                   the Stop nudge (no bd call, no idle mark).",
        silent_break: true,
        action: "Any number derived by summing `bd_calls` over the event log is wrong for \
                 every day before this: the adopter's 2026-08-30 summed to 570,989 while the \
                 largest total any process reached was 1,661, and one-shot commands cost \
                 1 to 4. Re-derive from lines written by this version; for older days, take \
                 the max `bd_calls` per command as that process's lifetime total. The bead's \
                 own 14.8-per-command figure was this artefact.",
    },
    SurfaceChange {
        id: "holdings-tense",
        since: "2026-09-05 (air-v7o)",
        headline: "`air holdings` tags name their tense: `uncommitted now, edited 3 min ago`, \
                   `uncommitted now, no edit journaled` (build or test output, with `verify in \
                   flight` when one is running), `journaled 6 h ago, clean now`. The report \
                   carries `at`, and `air status`'s `overlap:` lines print the same tags.",
        silent_break: true,
        action: "A script matching the old `[uncommitted]` / `[journaled]` tokens, or reading \
                 `overlaps` from `air status --json` as bare worker names, sees the new \
                 strings. `air holdings --json` gained `at`, `last_edit` and \
                 `verify_in_flight`. Read the tense: `uncommitted now` is true of `at` only; \
                 `journaled` is history with its age.",
    },
    SurfaceChange {
        id: "worker-deny-ask-owner",
        since: "2026-09-05 (air-bm3; owner ruling 2026-08-30)",
        headline: "Workers are denied `AskUserQuestion`, and the PreToolUse hook matcher \
                   carries it so an attempt is an event line.",
        silent_break: false,
        action: "A worker that needs the owner runs `air capture \"<question>\"`; the \
                 coordinator files it as a bead labelled `owner`, which `air status` counts \
                 and `air claim` refuses to workers. Re-run `air install --write` to pick up \
                 the matcher; workers launched by this binary carry the deny already. If a \
                 rule of yours told workers to ask the owner directly, point it at the \
                 capture.",
    },
    SurfaceChange {
        id: "worktrees-are-airs",
        since: "2026-09-05 (air-fdz)",
        headline: "`air worker <name>` creates `.claude/worktrees/<name>` itself (branch \
                   `worktree-<name>`) and copies the repo's `.worktreeinclude` files into it \
                   before claude starts; `air worker <name> --remove` removes it, refusing \
                   while it holds uncommitted work, a harness lock or a tmux session. claude \
                   is still handed the worktree by name, so its isolation is unchanged.",
        silent_break: false,
        action: "Nothing to change in how you launch. Check `.worktreeinclude` still gives a \
                 worktree that builds: Air matches its lines with git's own glob engine \
                 (`git ls-files --ignored` over `:(glob)` pathspecs), the same files the \
                 harness copied, but a negated line (`!x`) is reported and not honoured. A \
                 relaunch re-copies, so a worktree gets the current `.env`. Remove lanes with \
                 `air worker <name> --remove` rather than `rm -rf`; it names what is holding \
                 the worktree and keeps the branch.",
    },
    SurfaceChange {
        id: "install-reports-bd-prime",
        since: "2026-09-05 (air-b5k)",
        headline: "`air install` reports a `bd prime` hook left in `.claude/settings.json` \
                   (`STALE HOOK: SessionStart runs `bd prime --hook-json`...`) on every run \
                   until it is gone; the adoption doc no longer asks for a hand edit nobody \
                   re-checks.",
        silent_break: false,
        action: "Run `air install` and read any STALE HOOK line: delete the entry it names. \
                 The merge never removes another tool's hook, so the report is the only thing \
                 that will keep saying it is there.",
    },
    SurfaceChange {
        id: "digest-refusal-names-the-order",
        since: "2026-09-05 (air-yol)",
        headline: "The digest refusal, when a green is recorded at HEAD, says that committing \
                   the digest moves HEAD off that green and names the order: commit, `git \
                   merge main`, then `air record verify -- make verify` last.",
        silent_break: false,
        action: "Nothing to run. A rule of yours that explained this ordering by hand can \
                 point at the refusal instead; with no green at HEAD the message is as \
                 before.",
    },
    SurfaceChange {
        id: "install-refuses-unignored-air",
        since: "2026-09-05 (air-6di)",
        headline: "`air install --write` and `air init --write` REFUSE while `git check-ignore \
                   -q .air` fails, naming the fix. It used to be advice.",
        silent_break: true,
        action: "A repo that never ignored `.air/` now gets exit 2 from `install --write`: \
                 `echo '.air/' >> .gitignore` and re-run. The reason is the ledger: since \
                 `messages-table` it holds the text of every agent-to-agent message, and one \
                 `git add -A` would commit it. There is no bypass flag.",
    },
    SurfaceChange {
        id: "status-batch-ready",
        since: "2026-09-05 (air-80x.3)",
        headline: "`air status` lists the branches a verify lane may merge into its next \
                   batch, as `batch-ready: <worker> at <sha> (<beads>)`: head contains main, \
                   no green at that head, a `Bead:` trailer names a bead the worker holds. \
                   `--json` carries `batch_ready` and `not_batch_ready` with the reason.",
        silent_break: false,
        action: "Nothing to run. A verify lane scripts its merge from `air status --json`'s \
                 `batch_ready`; nothing pushes it, and a branch that is already green is \
                 landable instead and never listed here.",
    },
    SurfaceChange {
        id: "verification-lane",
        since: "2026-09-05 (air-80x.1, air-80x.6)",
        headline: "A verification lane. The close gate now accepts a green at a verified \
                   commit that contains `main` and every commit carrying the bead's trailer, \
                   recorded by any worker (air-80x.1): a worker closes on a lane's batch green \
                   with no verify run of its own. `.air/roles.md` gains a Verification lane \
                   section under Worker stating what Air records and refuses for it (air-80x.6).",
        silent_break: false,
        action: "Nothing to run; `air install --write` refreshes `.air/roles.md`. If your repo \
                 runs a lane, its flow (who the lane is, its cadence, what workers do instead \
                 of verifying) goes in your CLAUDE.md, not in roles.md. Without a lane nothing \
                 changes: a green at HEAD containing `main` still closes exactly as before.",
    },
    SurfaceChange {
        id: "install-lag-is-named",
        since: "2026-09-06 (air-d61)",
        headline: "`air doctor` and `air status` print one line when `.air/installed.json` \
                   records an older crate or surface version than the running binary: both \
                   versions, the count of unread notices, and the fix. The adopter's hooks ran \
                   0.2.18 for days on a record that said 0.1.0 / surface 2, with the ledger \
                   already migrated and the installed skills stale, and nothing said so. \
                   Printed, never refused; a repo with no record at all stays silent.",
        silent_break: false,
        action: "If the line appears, run `air install` to read the notices, then \
                 `air install --write`; the line goes away with the record.",
    },
    SurfaceChange {
        id: "stuck-deleted",
        since: "2026-09-06 (air-12k)",
        headline: "The `stuck` session state and attention condition are gone: no session \
                   reads it, no condition raises it, `air audit` no longer lists it, and \
                   `AIR_ATTENTION_STUCK_MIN` does nothing. It was set only by the \
                   PermissionRequest \
                   hook, which never arrives in auto mode — zero in 39,071 event lines.",
        silent_break: true,
        action: "A script or dashboard that filters `air status --json` for `stuck` now reads \
                 an empty set and says nobody is stuck, which is true and useless: it was \
                 already always empty. Replace it with the coordinator's heartbeat — \
                 `air status` on a timer — which is what actually caught every wedged worker; \
                 `.air/roles.md` says so, and `air install --write` refreshes it.",
    },
    SurfaceChange {
        id: "budgets-measured",
        since: "2026-09-06 (air-d75)",
        headline: "Every timing budget Air waits on records its elapsed time and whether it \
                   was hit, in a `budgets` object on the event line; `air audit` prints per \
                   budget the count, p50/p90/p99/max, hits and near misses with the fail \
                   direction beside each, plus an unpaired-hook count for the one budget that \
                   cannot record its own overruns. The SQLite busy timeout moves 200 ms -> 1 s \
                   and is now a recording `busy_handler`.",
        silent_break: false,
        action: "Nothing to run. Event lines gain one optional key, so a reader that ignores \
                 unknown fields is unaffected. Read `air audit`'s budget rows once a round: a \
                 non-zero `hits` on `git` or `sqlite-lock` is a hook that failed open, which \
                 is a refusal that did not happen. The harness's own Bash timeout is the one \
                 budget Air cannot record; see adopting-air.md §1 step 5.",
    },
    SurfaceChange {
        id: "no-harness-worktree-flag",
        since: "2026-09-06 (air-8gj)",
        headline: "`air worker` no longer passes `--worktree` (or `--tmux`) to claude. Air \
                   creates the worktree and starts claude IN it, so the harness's own worktree \
                   isolation is off; one PreToolUse check replaces it, denying an \
                   Edit/Write/MultiEdit whose RESOLVED path leaves the worker's worktree. In \
                   The adopter's record the harness block stopped no observed write to the main \
                   checkout and cost 455 refusals in five days, 388 of them (88%) with no git \
                   token in the command.",
        silent_break: false,
        action: "Nothing to run. Workers gain back the operations the harness was refusing \
                 (native builds, unattended commands) and lose one block; the tmux path is \
                 Air's on both routes, so `AIR_TMUX_MODE` and the iTerm2 native pane are gone \
                 with the flag. A Bash `cd ../..` is deliberately out of scope — the harness \
                 never caught that either — so if your repo needs it, that is a cwd-scoped \
                 command guard of your own.",
    },
    SurfaceChange {
        id: "metis-on-the-coordinator",
        since: "2026-09-06 (air-g5o)",
        headline: "`air coordinator` attaches Metis when `.claude/air.json` says \
                   `\"metis\": true`, which `air init` now writes by default: `--mcp-config` \
                   with metis's own server declaration, and `--plugin-dir` when \
                   `metis_plugin_dir` names a directory that exists. Workers never get it. \
                   The coordinator's appended prose gains a paragraph stating that Metis \
                   holds vision and initiatives while tasks stay beads and decisions stay in \
                   `docs/`, and `air status` prints how many beads declare no \
                   `initiative: <CODE>` line.",
        silent_break: false,
        action: "Nothing is refused: the count is a count, and a missing `metis` binary is one \
                 printed line, not a failed launch. Set `\"metis\": false` in \
                 `.claude/air.json` if your repo plans elsewhere. To get the plugin as well as \
                 the tools, set `metis_plugin_dir` to the `plugins/metis` directory of a metis \
                 checkout; Air will not guess that path, and a `--plugin-dir` pointing at \
                 nothing loads nothing silently.",
    },
    SurfaceChange {
        id: "adopter-check",
        since: "2026-09-06 (air-bpj)",
        headline: "`air adopter-check` refuses a tracked line naming an adopter. Names are \
                   read from `private/adopters.md` (one `name: <x>` line each), never from \
                   the binary, and the check SKIPS when that file is absent. Air's own tracked \
                   text now says \"an adopter\": an incident keeps its date, its count and its \
                   `air-` bead, and anything that quotes an adopter's files lives in an \
                   ignored `private/`.",
        silent_break: false,
        action: "Nothing to run, and nothing changes for a repo that does not use it. If YOUR \
                 repo is quoted in someone else's, the same shape works: `private/` in \
                 `.gitignore`, the names in `private/adopters.md`, and `air adopter-check` in \
                 your verify. Air will not tell you a name is missing — a check whose list is \
                 public would publish what it exists to hide.",
    },
    SurfaceChange {
        id: "init-scaffold",
        since: "2026-09-06 (air-ej4)",
        headline: "`air init --write` now scaffolds four empty-but-ready things a fresh repo \
                   needs and Air used to assume: a `Makefile` with a `verify` target that \
                   FAILS until it is edited, a `.worktreeinclude` with a comment header, and \
                   in the `CLAUDE.md` stub the hand-over sequence and the `Bead: <id>` trailer \
                   rule. Each is created ONLY when absent and never edited, and `air init` \
                   without `--write` lists what it would create.",
        silent_break: false,
        action: "Nothing changes for a repo that already has these: a present `Makefile`, \
                 `.worktreeinclude` or `CLAUDE.md` is not touched, and a `Makefile` with no \
                 `verify` target is REPORTED and still not edited. If you scaffold a new repo, \
                 know that its `make verify` exits 1 on purpose until you put a real check in \
                 it; that is what stops the first `air record verify` recording a green for an \
                 empty check.",
    },
    SurfaceChange {
        id: "close-asks-the-recorded-main",
        since: "2026-09-06 (air-9ij)",
        headline: "The close gate stopped expiring when main moves. `contains main` is now \
                   asked of the main the verify run was RECORDED over (schema v19, \
                   `verify_runs.main_sha`), not of main at the moment of the question, so a \
                   landing or an ordinary commit on main no longer retracts a batch green cut \
                   before it — an adopter's coordinator invalidated a whole batch with one \
                   prose commit. A bead whose every commit is already in main closes on the \
                   landing that put it there. `air land` is unchanged and still asks about \
                   current main.",
        silent_break: false,
        action: "Nothing to run; the migration is automatic, and rows written before it fall \
                 back to the old question, so re-record a green if an old one is refused. If \
                 your coordinator holds main still from the batch cut until every close is \
                 confirmed, that workaround can go. Landing ORDER still matters: a branch is \
                 landable only while it contains current main, so every write to main, a \
                 landing or the coordinator's own commit, costs every other branch its \
                 landability.",
    },
    SurfaceChange {
        id: "audit-reclaim-churn",
        since: "2026-09-06 (air-5nh)",
        headline: "`air audit` prints a `re-claim churn` section from the `claims` table: \
                   claims in the window, how many released, how many inside 60 s and 300 s, \
                   how many of those ended owner-gated, and the rows those counts read. \
                   Nothing new is recorded.",
        silent_break: false,
        action: "Nothing to run; the numbers come from claims your ledger already holds, so \
                 the first run covers your whole history if you pass `--since`. Read the rate \
                 as what it says: owner-gated releases inside a minute over claims, which is \
                 \"a worker took a bead it could not start\". The 10% threshold printed \
                 beside it is the boundary that would reopen wrapping bd's filing with an \
                 ordering edge; below it the ordering edge stays prose. If your workers do \
                 not release with a reason naming the owner, the rate under-reads, which is \
                 the safe direction but worth knowing.",
    },
    SurfaceChange {
        id: "epic-ready-to-decompose",
        since: "2026-09-06 (air-84u)",
        headline: "`air status` names each ready epic that has no open child, as `epic ready \
                   to decompose: <id> (0 open children, N closed)`, and `.air/roles.md` states \
                   decomposition as the coordinator's standing duty rather than as a property \
                   of a good queue. Nothing refuses and nothing pushes; the count was already \
                   printed, this says which epic it is about.",
        silent_break: false,
        action: "Nothing to run; `air install --write` refreshes `.air/roles.md`. It costs one \
                 `bd list --parent` per READY epic, so a tick with no ready epic pays nothing \
                 and the cost only appears in the state the line exists to report.",
    },
    SurfaceChange {
        id: "stopped-sessions",
        since: "2026-09-06 (air-1n3)",
        headline: "`air install --write` now adds `air hook` on `Notification` and \
                   `StopFailure`, and a session the harness stops is recorded on its row \
                   (`stopped_at`, `stopped_kind`, `stopped_text`, schema v20) and printed by \
                   `air status` as `STOPPED at <t>` with the reason. Nothing is refused, \
                   woken or relaunched. The SessionStart hook also says one sentence, once \
                   per session, telling it to create a recovery wake.",
        silent_break: false,
        action: "Run `air install --write` to get the two new hook entries; without them \
                 nothing records a stop. Then READ THE KIND before acting on one: \
                 `quota_auto_resume_fired` means the harness is bringing that session back \
                 and typing at it CANCELS the recovery, while `quota_auto_resume_stale`, \
                 `quota_auto_resume_disabled` and `stop_failure` mean nothing is coming. If \
                 your fleet has a rule that nudges a silent worker, narrow it to the kind: on \
                 2026-09-06 five of seven sessions on one machine were recovering on their \
                 own while two were not, and silence read identically for both. Requires a \
                 harness that sends these events; on one that does not, the columns stay \
                 NULL and nothing changes.",
    },
    SurfaceChange {
        id: "ancestor-deadlock-named",
        since: "2026-09-06 (air-btz)",
        headline: "`air status` names a bead blocked by one of its own ancestors, with the \
                   edge and the `bd dep remove` that clears it. Such a bead can never become \
                   ready — the ancestor cannot finish until its descendants do — and bd shows \
                   it as \"not ready yet\" like any queued bead. bd 1.2.2 refuses the edge on \
                   nine routes but NOT from `bd create --graph` or `bd create --parent X \
                   --deps <ancestor>`, because its guard is a parent-child row on the pair \
                   plus a dotted-id prefix test, not an ancestor walk.",
        silent_break: false,
        action: "Nothing to run, and silent unless a repo has the shape. It costs one \
                 `bd list --status …` per tick, plus one `bd dep list` only when a bead that \
                 has a parent also has an edge. If you file waves with `bd create --graph`, \
                 read `bd dep tree <epic> --json` after each one for an edge from a child to \
                 ANY ancestor: `bd dep cycles` does not report this shape, because the \
                 hierarchy is definitional rather than an edge.",
    },
    SurfaceChange {
        id: "adopters-declared",
        since: "2026-09-06 (air-jsz)",
        headline: "`air adopter-check` no longer skips silently when it has no names. \
                   `.claude/air.json` gains `\"adopters\"`: declared true with no \
                   `private/adopters.md` is now a REFUSAL naming the file to write; \
                   undeclared with no list still skips, which is the clone-with-no-adopter \
                   case. The list is read from the MAIN checkout, beside the declaration, so \
                   a worktree's copy cannot disagree with it. `air init` writes \
                   `\"adopters\": false`.",
        silent_break: true,
        action: "If your repo quotes an adopter, set `\"adopters\": true` AND write \
                 `private/adopters.md` in the main checkout BEFORE upgrading, or your next \
                 verify goes red. If it quotes nobody, do nothing: the default is false and \
                 the behaviour is unchanged. Why this changed: the check ran for a whole \
                 round here having never once been given a list — the file was absent \
                 everywhere and every green verify printed `Skipped`, so the one mechanism \
                 guarding the no-adopter-content rule would have passed over any leak. A \
                 count of zero firings meant nothing, because the input never arrived. Check \
                 your own verify output for that line before assuming yours has ever run.",
    },
    SurfaceChange {
        id: "red-run-output-kept",
        since: "2026-09-06 (air-5ik)",
        headline: "`air record` keeps what a NON-GREEN run printed: the last 64 KiB, both \
                   streams in arrival order, at `.air/logs/<run-id>.log`, with the path on \
                   `verify_runs.log_path` — a column that has existed since schema v1 and was \
                   NULL on every row ever written. `air record` prints the path and \
                   `air status` puts it beside a worker's `not green`. A green run writes \
                   nothing.",
        silent_break: false,
        action: "Nothing to run; `.air/` is already gitignored. The store is bounded at write \
                 time by COUNT — 20 logs, so 1.25 MB at most, ever — and prunes itself, so \
                 unlike `.air/events/` there is nothing to collect and no `gc` window to \
                 choose. If your build prints more than 64 KiB you get its tail, which is \
                 where a verify fails.",
    },
    SurfaceChange {
        id: "stop-names-the-command",
        since: "2026-09-06 (air-avj)",
        headline: "The Stop hook no longer prints the flow-dependent repairs. It stated `git \
                   merge main` and `air record verify -- make verify`, which under a verify \
                   lane are the two things the lane exists to prevent — merging moves the head \
                   off the sha the lane cut at, and recording a green is the lane's job. It \
                   now states the same FACTS and names `air handover`, which reads your repo's \
                   flow and prints the repair it calls for. Every other fix, and every CLI \
                   surface, is unchanged.",
        silent_break: false,
        action: "Nothing to run. If anything of yours greps the Stop hook's \
                 `additionalContext` for `git merge main`, it will not find it; the check \
                 names and details are unchanged, and `air handover --json` still carries \
                 every `fix` verbatim, plus a new `flow_dependent` flag per check. Air does \
                 NOT read `verify_lane` for this: that key stays yours, and a hook branching \
                 on it would be a second copy of a decision `air handover` already makes.",
    },
    SurfaceChange {
        id: "claim-records-the-resolved-id",
        since: "2026-09-06 (air-x1ha)",
        headline: "`air claim` records the id BD RESOLVED, not the string that was typed, so a \
                   prefix claim and a full-id claim produce identical rows. And the status \
                   reconcile now tells an id bd never had from one bd no longer holds: only \
                   the second releases the row, and a kept one is reported by id. Before this, \
                   a worker typed a prefix, bd claimed the full id, Air's row went under the \
                   prefix, and the next reconcile released the claim while the work continued.",
        silent_break: false,
        action: "Nothing to run, and nothing is released that was not released before — this \
                 only stops releases. If you have rows recorded under a prefix from before, \
                 `air status` now names them under \"kept N claim(s) bd could not resolve\" \
                 instead of silently dropping them: re-claim under the id bd knows, or \
                 `air release <id> --reason unknown`. A `bd show` that TIMES OUT no longer \
                 releases anything either, which it used to for every claim not in the \
                 in-progress list.",
    },
    SurfaceChange {
        id: "version-says-the-build",
        since: "2026-09-06 (air-dwq5)",
        headline: "`air --version` now prints the commit the binary was built from and its \
                   surface version, `air --version --json` emits JSON instead of the same \
                   plain string, and `air doctor` and `air status --json` carry the same \
                   object. Air embedded the build, stored it in `installed.json` and read it \
                   for the install-lag check, and told nobody — so one crate version covered a \
                   whole round of behaviour changes and two binaries with different behaviour \
                   reported the same string.",
        silent_break: true,
        action: "If anything of yours parses `air --version --json`, it was getting `air \
                 0.2.19` and now gets `{\"name\":\"air\",\"version\":…,\"built_from\":…,\
                 \"surface_version\":…}`; the plain `air --version` gained a suffix on the \
                 same line. `air status --json` gains a third top-level key, `air`, beside \
                 `snapshot` and `attention`. Attribute a log line or a session to a build with \
                 `built_from`, not with the version: lanes cut no release rows mid-round, so \
                 the version does not move when behaviour does.",
    },
    SurfaceChange {
        id: "batch-members-are-what-it-took",
        since: "2026-09-06 (air-vsvt)",
        headline: "A verify run records, per worker branch, the sha the batch actually TOOK \
                   from it, not that worktree's head at recording time. A worker that commits \
                   between the lane's merge and the lane's `air record` used to drop out of \
                   its own batch's member list, and because the list is written to the row it \
                   stayed wrong. An adopter's lane saw that five times in one night. The \
                   reporting path is unchanged: it always read what was recorded.",
        silent_break: false,
        action: "Nothing to run, and nothing changes for a batch whose branches sat still. \
                 Member shas on `verify_runs.members` and on landing rows are now the merged \
                 sha rather than a later head, so anything of yours that compared a member \
                 sha against a worktree's current head will now see them differ, which is the \
                 point. Rows written before this keep whatever they recorded.",
    },
    SurfaceChange {
        id: "discharged-clause-names-its-lookup",
        since: "2026-09-06 (air-rud0)",
        headline: "A landing's acceptance report prints `ok (the merge changed <path>) — \
                   <clause>` where it printed a bare `ok`. The verdict was already honest and \
                   its reason was already on the line beneath; what was wrong is that a \
                   reader scanning the verdict column saw a REASON on the branches Air could \
                   not read and a bare tick on the branch it could, so the weaker claim wore \
                   the stronger form. `ok` means a lookup matched, never that a clause's \
                   substance was checked.",
        silent_break: true,
        action: "If anything of yours greps a landing's output for `ok ` followed by the \
                 clause text, the lookup now sits between them, and the discharged clause is \
                 one line rather than two. `MISS` and `?` are unchanged, reason on the line \
                 beneath. `--json` is unchanged entirely: `Verdict::Discharged` has carried \
                 `how` since it was written, which is why this cost no new fact.",
    },
    SurfaceChange {
        id: "session-journal",
        since: "2026-09-06 (air-3xww)",
        headline: "`.claude/air.json` gains `journal_dir`, `air init` scaffolds that directory \
                   with a README, and `.air/roles.md` tells both roles to keep one file per \
                   session there. **Air reads none of it**: no gate, no condition, no count, \
                   no check that a session wrote one.",
        silent_break: false,
        action: "Nothing to run and nothing refuses. Set `journal_dir` if you want the habit \
                 and pick your own path; leave it out and nothing happens. What it is for: a \
                 finding that is neither about the bead a worker holds nor worth the \
                 coordinator's inbox has nowhere to go today, so it lives in a message and \
                 dies with the recipient's session — a coordinator here hit an account limit \
                 mid-round with the round's best material only in its memory of messages. The \
                 distinction worth passing on: a capture says somebody should ACT and every \
                 one is triaged; these entries say nobody should, which is why routing them to \
                 captures is the wrong advice.",
    },
    SurfaceChange {
        id: "journal-branch-lands-without-a-bead",
        since: "2026-09-06 (air-kexg)",
        headline: "A branch whose only commits are session-journal entries (everything under \
                   the repo's `journal_dir`) lands with no `Bead:` trailer. A journal entry is \
                   not work on a bead, so it is the one commit a worker legitimately writes \
                   that names none, and such a branch was refused with \"no commit declares a \
                   bead\". A range MIXING journal commits with anything else is unchanged and \
                   still needs a trailer.",
        silent_break: true,
        action: "If anything of yours reads `air status --json`'s `landable`, its `bead` field \
                 is now `null` for a journal-only branch rather than always a string; every \
                 other row is unchanged. A repo that declares no `journal_dir` has no journal \
                 case and nothing changes for it. `air land <bead-id>` cannot select a journal \
                 branch, since there is no id to name it by — use `--worker` or `--all`.",
    },
    SurfaceChange {
        id: "close-reason-file",
        since: "2026-09-07 (air-lyjr)",
        headline: "`air close <id>... --reason-file <path>` records the file as the reason, \
                   untruncated, on every id named — the same one reason `--reason` already \
                   applied to all of them. `--reason` stays; exactly one of the two, and both \
                   or neither is refused naming both. A path that cannot be read is an error, \
                   never an empty reason.",
        silent_break: false,
        action: "Nothing to re-run. This is air-45pw one command over, and it bites harder \
                 here: if your repo asks a close to carry proof — a command and its output — \
                 then the command demanding the longest argument was the one refusing it. The \
                 harness classifies a long `--reason \"...\"` by the shape of the command line \
                 and refuses it, and the obvious next move is to shorten the proof. Point your \
                 rules at `--reason-file` for anything past a line or two. Observed on `bd \
                 close` at ~2,500 characters, where bd's own `--reason-file` took the identical \
                 text on the next attempt.",
    },
    SurfaceChange {
        id: "capture-file",
        since: "2026-09-06 (air-45pw)",
        headline: "`air capture --file <path>` files a whole file as the capture text, \
                   untruncated. The positional stays and most captures are still one-liners; \
                   exactly one of the two, and passing both or neither is refused naming both.",
        silent_break: false,
        action: "Nothing to re-run and nothing refuses that did not before. Worth knowing if \
                 your workers capture through the CLI rather than the `air_capture` MCP tool: \
                 a finding long enough to be worth writing goes through the harness's command \
                 classifier as a command line, and one was refused for its shape — the worker \
                 shortened the finding in order to file it. A shortened capture looks exactly \
                 like a capture, so the loss is silent. Point your rules at `--file` for \
                 anything longer than a line. A fleet capturing through the MCP tool never \
                 sees this, which is why it took an adopter to find it.",
    },
    SurfaceChange {
        id: "undischarged-clause-wording",
        since: "2026-09-06 (air-k6uh, air-jy99)",
        headline: "The `landed-not-closed` line no longer says a merge CONTRADICTS a clause, \
                   and no longer tells you to reopen the bead. It reports what the lookup \
                   established — an acceptance clause naming a file the merge did not change — \
                   and hands the decision back. Both renderers changed: `air status` and `air \
                   land`'s summary.",
        silent_break: true,
        action: "Two things. Anything of yours matching `CONTRADICTS` or `reopen` in this \
                 line's text stops matching; the condition kind (`landed-not-closed`) and the \
                 ledger row are unchanged, so match on the kind. And re-read any bead you \
                 acted on because of this alert: of nine standing firings here, SIX were \
                 clauses that held — three satisfied in a commit other than the merge, three \
                 naming a path the clause only mentions. The line asserted a contradiction it \
                 had not established. It also prescribed reopening, which a flow that says \
                 \"closed is closed\" forbids; Air reads no repo's flow and now names no \
                 action here, so what to do about an undischarged clause is yours to decide.",
    },
    SurfaceChange {
        id: "closed-not-landed",
        since: "2026-09-07 (air-gazh)",
        headline: "New attention condition `closed-not-landed`: a bead bd has closed whose \
                   commits are in no tree but its author's worktree. Pushed on the channel and \
                   printed by `air status` (`closed, not landed: <worker> (<bead>) at <sha>`), \
                   with `closed_not_landed` in `--json`.",
        silent_break: false,
        action: "Nothing to run. Read it if your fleet BATCHES: the state forms with no error \
                 anywhere in it — a worker closes on a batch green, the coordinator commits to \
                 main, that batch stops containing main, later cuts red or are killed, and the \
                 branch simply never lands. An adopter had six at once, found while answering \
                 an unrelated question. Nothing looks wrong from inside the worktree (bead \
                 closed, branch green, tree clean), which is why the person best placed to \
                 notice is the last who will. A fleet that lands each branch soon after its \
                 close never reaches it, and a quiet line here measures your landing cadence \
                 rather than the check.",
    },
    SurfaceChange {
        id: "capture-head",
        since: "2026-09-07 (air-6dj4)",
        headline: "A capture records the worker's HEAD sha at the time it was written, and \
                   `air inbox` renders it (`<id>  <time>  <worker> at <sha8>  <text>`). Ledger \
                   schema v21 adds `captures.head_sha` and `captures.head_absent`.",
        silent_break: true,
        action: "Re-read `air inbox` output if anything of yours parses it by column: the \
                 worker field is now followed by ` at <sha8>`, or by ` [no head: <why>]` where \
                 Air looked and found none, or by nothing at all on a row written before v21. \
                 Those three are deliberately distinct — a row from before this change did not \
                 have a head looked up, which is a different claim from having none, and \
                 collapsing them would make old rows assert something nobody established. \
                 `--json` gains `head`, either `{\"At\": \"<sha>\"}` or `{\"Absent\": \
                 \"<why>\"}` or null. Why it exists: a capture's time is on the row and its \
                 subject is in the body, and the body is what gets quoted onward, so \"the \
                 batch is red\" used to arrive somewhere else with no way to say which batch.",
    },
    SurfaceChange {
        id: "craft-notes-on-the-bead",
        since: "2026-09-07 (air-u3l7)",
        headline: "`roles.md`'s Coordinator section now names the route: put craft notes on the \
                   bead with `bd comment <id> --file <notes>`, not in the message that names \
                   the bead at a worker. The rule that naming a bead reserves nothing is \
                   unchanged; what is added is what to do instead.",
        silent_break: false,
        action: "Re-run `air install --write` to refresh `.air/roles.md`. Worth reading if your \
                 coordinator dispatches by message: the old line stated only the CONSEQUENCE \
                 (your reservation will not hold), and between 2026-09-05 and 2026-09-07 it \
                 was read, agreed with and worked around five times — two beads lost at one \
                 repo, two dispatched twice at another, and once the Stop hook itself told an \
                 idle worker to claim a bead already spoken for, from correct inputs. No new \
                 state, nothing refuses, and the hook is unchanged: a ready bead is available \
                 again once nothing is reserved outside the bead.",
    },
    SurfaceChange {
        id: "digest-must-be-tracked",
        since: "2026-09-06 (air-ahlf)",
        headline: "The hand-over gate REFUSES a digest git does not track, under its own check \
                   name. It used to read the directory, so a digest you wrote and never added \
                   satisfied it — a file only your worktree has, offered as proof to somebody \
                   who cannot see it. `digest_for_bead` now answers Missing, Untracked or \
                   Tracked, and the two get different fixes because \"write one\" and \"add the \
                   one you wrote\" are different repairs.",
        silent_break: false,
        action: "This is a NEW REFUSAL you can hit: commit the digest, not just write it. A \
                 tracked digest beside an untracked stray still passes. If your flow writes \
                 digests late, move the commit before the close.",
    },
    SurfaceChange {
        id: "handover-attempts-count-refusals-only",
        since: "2026-09-06 (air-zqmi)",
        headline: "`handover-not-green` counted SUCCESSES. The gate stamped an attempt on \
                   every hand-over command it saw, and that condition reads the counter — so a \
                   worker whose closes all passed was reported to the whole fleet as having \
                   failed. Only a hand-over that did NOT go through is an attempt now, and a \
                   pass CLEARS the counter, because otherwise one early refusal keeps firing \
                   after a clean close. `air handover`, the diagnostic, never stamps at all \
                   (air-eiv): running the query used to raise the alarm it was meant to \
                   diagnose.",
        silent_break: true,
        action: "If you discounted this condition, or told your workers to, STOP discounting \
                 it — its firings mean what they say now. On an older binary it fires for \
                 refusals that never happened, and there are TWO tells: a worker whose closes \
                 all passed carrying `handovers N` on `air status`, and a worker that runs \
                 `air handover` BY HAND, which stamped once per run before air-eiv — an \
                 adopter found the second at 77 runs in one worktree. To tell a diagnostic \
                 stamp from a genuinely refused close, join the claim's `last_handover_at` \
                 against your events stream for a `bd close` at that time: a real attempt has \
                 one beside it and a diagnostic stamp does not. Nothing to run; the counter \
                 clears on the next passing hand-over of that bead.",
    },
    SurfaceChange {
        id: "status-json-says-why-not-landable",
        since: "2026-09-06 (air-72t7)",
        headline: "`air status --json` carries `land_skipped` and `land_errors` beside \
                   `landable`, all three from ONE selection so they cannot disagree. It used \
                   to say which branches can land and never why the others cannot, and a real \
                   failure inside selection — git or the ledger — reached no caller at all and \
                   read as an empty queue.",
        silent_break: false,
        action: "Nothing to run and nothing existing changed; the human rendering is \
                 untouched. A script that inferred \"not in `landable` means not landable\" \
                 can now read the check, the detail and the fix, and should treat a non-empty \
                 `land_errors` as \"could not tell\" rather than as an empty queue.",
    },
    SurfaceChange {
        id: "handover-refusal-names-which-not-green",
        since: "2026-09-06 (air-hgi9)",
        headline: "The hand-over refusal spelled four distinct not-green states identically \
                   and named what was missing in only one. Each now states the fact that \
                   distinguishes it — no green recorded at all, a green that predates the \
                   bead's newest commit, a green at a head that does not contain main, a green \
                   recorded over — with the fixing line carried beside the reason rather than \
                   re-derived. Two of the four have OPPOSITE correct responses, which is what \
                   made one sentence for four states expensive.",
        silent_break: false,
        action: "Nothing to run. Anything of yours matching the old single sentence will not \
                 match; the check names are unchanged and `air handover --json` carries each \
                 reason and fix verbatim.",
    },
    SurfaceChange {
        id: "land-bd-budget-by-id-count",
        since: "2026-09-06 (air-fzv)",
        headline: "`air land`'s bd budget is `10 s + 2 s per id` rather than a flat 10 s for \
                   the whole set, and the refusal names the id count, the budget and the \
                   override. Measured here: `bd show` with 14 ids took 21.27 s against 1.94 s \
                   for one, so a landing carrying many beads timed out on a budget that was \
                   never sized for it.",
        silent_break: false,
        action: "Nothing to run. `AIR_BD_TIMEOUT_MS` overrides the whole budget if your bd is \
                 slower or faster than ours; the refusal names it when you hit it.",
    },
    SurfaceChange {
        id: "batch-landing-attributes-every-bead",
        since: "2026-09-05 (air-80x.2)",
        headline: "`air land --worker <lane>` lands a batch branch as ONE landing and \
                   attributes every bead the merged branches carry by `Bead:` trailer, with \
                   the member heads recorded on the landing row. Without this a lane's batch \
                   landed as one branch and the beads its members carried were attributed to \
                   nothing.",
        silent_break: false,
        action: "Nothing to run, and nothing changes for a repo with no lane. If you run one, \
                 put a `Bead:` trailer on every commit that does a bead's work — that trailer \
                 is the whole attribution, and a commit without one is attributed to nothing.",
    },
    SurfaceChange {
        id: "red-batch-is-reported-by-member",
        since: "2026-09-05 (air-80x.4), unbounded 2026-09-06 (air-cyf)",
        headline: "A red verify at a batch head is reported BY MEMBER: `air record` prints \
                   `batch red at <sha> (<lane>): members …; nothing lands on it, the lane \
                   splits by hand`, and `air status` repeats the newest standing one until a \
                   later green carries every member. air-cyf then removed the 20-run window \
                   that decided \"standing\": a batch that stayed red across 20 further runs \
                   silently stopped being reported, and a dropped report looked exactly like a \
                   fixed one.",
        silent_break: false,
        action: "Nothing to run, and nothing lands, closes or claims differently on a red. If \
                 you run a lane, expect the line to persist rather than age out — that is the \
                 fix, not noise, and it clears when a green carries every member.",
    },
    SurfaceChange {
        id: "land-looks-at-main-before-reporting",
        since: "2026-09-06 (air-htmn)",
        headline: "A fast-forward that TIMED OUT after succeeding was reported and recorded as \
                   \"main is untouched\". `git merge --ff-only` updates the ref atomically and \
                   Air kills the child on expiry, which does not undo a ref update, so an \
                   error there means \"I stopped waiting\" and never \"it did not happen\". Air \
                   now asks `merge-base --is-ancestor` and reports what it finds. The landing \
                   row is the half that mattered: the incident row carried an empty \
                   `merge_commit` and no `landed` row at all, so the record of that landing was \
                   ABSENT and the beads it carried were attributed to nothing.",
        silent_break: true,
        action: "Nothing to run. If your landings rows have a `refused / fast-forward` with an \
                 empty `merge_commit` while main carries the merge, that is this bug and the \
                 beads on it are unattributed — Air has no command that records a landing after \
                 the fact, so re-attribute by hand if you need it. When the look itself fails, \
                 NOTHING is recorded: the in-flight row already says a landing started and has \
                 not reported, which is precisely true.",
    },
    SurfaceChange {
        id: "idle-without-claim-respects-a-running-verify",
        since: "2026-09-06 (air-t6ap)",
        headline: "`idle-without-claim` no longer fires for a worker whose own verify is in \
                   flight. It offered an adopter's verify lane 58 claimable beads 945 seconds \
                   into a batch verify, with `air status` printing `verify in flight` three \
                   lines above — and a lane that claims a bead mid-batch cannot cut the batch, \
                   so the remedy was the failure it exists to prevent.",
        silent_break: false,
        action: "Nothing to run. The condition still fires for a genuinely idle worker with \
                 claimable work, and somebody else's verify does not silence it. If you added \
                 a rule telling your coordinator to ignore this condition for a lane, you can \
                 drop it.",
    },
    SurfaceChange {
        id: "selftest-json-is-only-the-array",
        since: "2026-09-06 (air-e21v)",
        headline: "`air selftest --json` emitted a stray line before the array, so it did not \
                   parse — and `air selftest --prove`, which reads it, reported EVERY declared \
                   mutation as broken. The evidence tool was dead by construction: 82 broken \
                   before, 80 proven and 3 vacuous and 1 genuinely broken after. Cause was a \
                   tee whose sinks were this process's stdout and stderr, right for `air record` \
                   and wrong inside a probe whose parent emits JSON.",
        silent_break: true,
        action: "Re-run `air selftest --prove`: on an older binary its verdict was \
                 uninformative rather than bad news, and anything of yours that skipped past a \
                 leading non-JSON line can stop. The first byte is `[` again.",
    },
    SurfaceChange {
        id: "peer-warning-dates-its-holders",
        since: "2026-09-06 (air-et0o)",
        headline: "The file-overlap warning dated nothing, so a live concurrent edit and a \
                   fortnight-old journal entry read identically: `is also being edited by w3` \
                   asserted a present tense the ledger cannot know. It now reads `is also \
                   journaled by w3 (14 d ago), w2 (3 min ago)`, following `air holdings`, which \
                   has dated its holders since air-v7o.",
        silent_break: false,
        action: "Nothing to run. On an older binary the warning fires the same way on a \
                 sixteen-day-old row as on a live one, and the only way to tell was `air \
                 holdings --file <path>` — which is the check the warning exists to save. \
                 Upgrading is the repair; wrapping the warning locally is not.",
    },
    SurfaceChange {
        id: "status-names-a-session-with-no-transcript",
        since: "2026-09-06 (air-3jv5)",
        headline: "A hook invocation from a demonstration or a fixture writes a session row \
                   indistinguishable from a real session, and it fired channel conditions. Air \
                   REPORTS rather than refuses: the row and its event line are still written, \
                   `air status` names it NO TRANSCRIPT, and it is never announced as a worker \
                   joining. Refusing was rejected deliberately — it needs Air to tell a real \
                   session id from a fake one, and losing a live worker from `air status` the \
                   day a harness omits the field is strictly worse than showing a synthetic \
                   row.",
        silent_break: false,
        action: "Nothing to run. If `air status` names a session NO TRANSCRIPT, it is a row no \
                 session is behind; the SQL to list them is in the line itself.",
    },
    SurfaceChange {
        id: "epic-count-drops-the-instruction",
        since: "2026-09-07 (air-3vkg)",
        headline: "The `ready:` line's epic count no longer says \"to decompose\": it reads \
                   `N epic(s), not claimable`. That count is every epic in the ready set, \
                   decomposed or not, so the old phrase told a coordinator to decompose epics \
                   that were already at their frontier — one adopter audited all six of theirs \
                   on the strength of it and none needed anything. The instruction now sits \
                   only on the line that can tell, `epic ready to decompose: <id> (0 open \
                   children, N closed)`, which is computed from the children and is the \
                   number the phrase always meant. This SUPERSEDES the wording quoted in the \
                   `ready-split-epics` notice above; that notice is left as written, since it \
                   was true when it shipped.",
        silent_break: true,
        action: "Anything of yours that greps `air status` for `epic(s) to decompose` finds \
                 nothing now and will not error. The count is unchanged in value and in \
                 source; only the words moved. Read the per-epic line for the action, and \
                 note it is absent on a cached status — the same line ends `(cached; bd not \
                 called this tick)` there, because the children calls it needs were not made.",
    },
    SurfaceChange {
        id: "handover-answers-landing-and-batch",
        since: "2026-09-06 (air-33rn, air-hpp8)",
        headline: "`air handover` now answers two questions a worker previously had to ask \
                   somebody. It always prints a `landing:` line saying what `air land` would \
                   say about this branch, read from the same selection the command runs; and \
                   when a standing RED batch has this branch among its members it prints a \
                   `batch:` line naming the batch sha, the lane, the time and where the lane's \
                   output is. Both are lookups over values Air already computes, not second \
                   copies of a decision. Workers are denied `air land` and were told about a \
                   red batch only by the lane remembering to message each member, which in one \
                   fleet reached everyone but one member twice in a night.",
        silent_break: false,
        action: "Nothing to run, and nothing existing changed: `--json` gained a `landing` key \
                 always and a `batch` key only when there is a batch to name, alongside the \
                 unchanged `pass`, `block`, `message` and `missing`. Read the `batch:` line \
                 as one-directional — its ABSENCE is not a statement that your branch was not \
                 in a batch, because Air knows membership only from what the run recorded, and \
                 a run that recorded none is indistinguishable from no batch at all.",
    },
    // ---- 2026-09-07, air-bh6n --------------------------------------------------------
    // Three notices the sweep owed and did not cut, because it excluded `roles.md` by CLASS:
    // prose, in a file that ships whole, refreshed by `air install --write`. That reasoning is
    // about the ARTIFACT and presupposes the adopter runs install — which under a freeze they
    // do not, and nothing prompts them to. The test that should have been applied is about the
    // EFFECT: does a reader of the shipped text now do, or believe they may do, something
    // different? `roles.md` is embedded in the binary and written to `.air/roles.md`, so a
    // change to it that alters a permission or a duty is a surface change like any other.
    SurfaceChange {
        id: "lane-between-batches-conditional",
        since: "2026-09-06 (air-4noi), notice cut 2026-09-07 (air-bh6n)",
        headline: "The verify lane's permission to hold a bead BETWEEN batches is conditional: \
                   it may, `if its worktree survives the cut`. A lane that resets hard to main \
                   on every cut would hold that bead in a tree it is about to wipe; a lane that \
                   merges main forward and integrates on a throwaway branch keeps it. Which of \
                   the two a repo runs is its own flow, and Air reads neither.",
        silent_break: true,
        action: "Read `.air/roles.md`'s verification-lane section again, and if it says only \
                 \"between batches it may hold one\" with no condition, your copy predates \
                 this and your lane has been reading an unconditional permission. **The \
                 decision is yours and you may not have known it was**: whether your lane can \
                 hold a bead between batches depends on how your lane makes a batch contain \
                 main. `air install --write` refreshes the file; if you are under an install \
                 freeze, read this notice as the change itself. Nothing refuses either way.",
    },
    SurfaceChange {
        id: "coordinator-context-is-the-channel",
        since: "2026-09-06 (air-zth), notice cut 2026-09-07 (air-bh6n)",
        headline: "`roles.md` tells the coordinator its context IS the channel the owner and \
                   every worker reach, so long reads, dry runs and analyses go to a background \
                   agent with a file deliverable while the filing and the deciding stay with \
                   the coordinator.",
        silent_break: false,
        action: "Nothing refuses and nothing is measured. Worth passing to your coordinator \
                 because it came from a recorded outage rather than from taste: on 2026-09-06 \
                 a coordinator inside a twenty-minute read was the only path to the owner and \
                 to four workers, and messages queued behind it. Re-run `air install --write` \
                 to refresh `.air/roles.md`. Cut on a WIDER test than the sweep's, and \
                 deliberately: nothing an adopter already acts on becomes false here, so the \
                 sweep's \"a fact they act on becomes false\" would exclude it — but a reader \
                 of the shipped text now delegates work they previously did inline, which is a \
                 duty changing, and that is the line (air-bh6n).",
    },
    SurfaceChange {
        id: "digest-without-a-trailer-under-a-lane",
        since: "2026-09-06 (air-ahl), notice cut 2026-09-07 (air-bh6n)",
        headline: "Under a verify lane, commit the digest WITHOUT a `Bead:` trailer once the \
                   lane has cut a batch at your head. The batch green must contain every commit \
                   that NAMES the bead, so an untrailered commit never joins that set: your \
                   head moves and the close still passes against the batch you were cut at.",
        silent_break: false,
        action: "The `air-ahlf` notice above covers the refusal (a digest git does not track is \
                 not proof) and stops there; this is the other half of the same `roles.md` \
                 change, and it is the half a lane needs. Trailer the work, not the digest, \
                 once your batch is cut. Appended rather than folded into that row because a \
                 shipped row is never edited.",
    },
    SurfaceChange {
        id: "role-is-the-launchers",
        since: "2026-09-14 (owner ruling)",
        headline: "Who may run `air land`, `air close`, `air release --worker`, and claim an \
                   `owner` bead now comes from `AIR_ROLE`, never from the directory the command \
                   runs in. The hook's fence and Stop advice follow the same rule, and `air land` \
                   always moves main in the main checkout wherever it is run from.",
        silent_break: true,
        action: "Sessions started with `air worker` and `air coordinator` need nothing. A worker \
                 started any other way must set AIR_ROLE=worker, or it is treated as the owner \
                 and may land. A shell with no AIR_ROLE is the owner.",
    },
    SurfaceChange {
        id: "batch-ready-behind-main",
        since: "2026-09-25 (owner ruling, from an adopter's fleet protocol)",
        headline: "A branch behind `main` is now batch-ready: `air status` no longer drops it \
                   with `behind-main`. The lane merges main forward at the cut, so a landing no \
                   longer takes every waiting branch out of the queue until its worker re-merges.",
        silent_break: false,
        action: "Workers under a lane stop merging main just to re-enter the queue. A lane that \
                 cuts from the listed shas already merges main first; one that assumed every \
                 member contained main must now drop, and name, a member that conflicts with it.",
    },
    SurfaceChange {
        id: "nudge-skips-the-lane",
        since: "2026-09-25 (from an adopter's fleet protocol)",
        headline: "The Stop hook no longer offers ready beads to the verification lane, the \
                   session `air lane` started (AIR_ROLE=lane). The lane claims no bead.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "roles-is-the-protocol",
        since: "2026-09-25 (owner ruling)",
        headline: "`.air/roles.md` now carries the whole fleet protocol: both closing sequences \
                   (with and without a lane), proof and `--reason-file`, the lane's loop, conflict \
                   facts, and landing a batch with `air land --worker <lane>`. An unclear \
                   acceptance is no longer a reason to ask the owner; the coordinator rewrites it.",
        silent_break: false,
        action: "Delete the repo's own copies of that protocol from CLAUDE.md and rules files. \
                 Keep only what is the repo's: domain rules, its verify and precheck commands, \
                 worktree setup, shared resources and leases, and any test-state reset.",
    },
    SurfaceChange {
        id: "leases-declared",
        since: "2026-09-25 (from an adopter's fleet protocol)",
        headline: "`.claude/air.json` can declare which commands need which lease, \
                   `\"leases\": {\"runtime\": [\"make api*\", \"adb *\"]}` in the deny-rule \
                   pattern syntax. Air's PreToolUse hook then refuses such a command from a \
                   worker that does not hold the lease (advises the coordinator; never the \
                   owner), and `air lease needs \"<cmd>\"` says what a command needs.",
        silent_break: false,
        action: "Optional; nothing changes without the key. A repo with its own lease guard can \
                 move its patterns into `leases` and retire the guard, keeping one store and one \
                 checker. Run both only while comparing them with `air lease needs`.",
    },
    SurfaceChange {
        id: "precheck-is-a-run",
        since: "2026-09-25 (owner)",
        headline: "`air record precheck -- <cmd>` records a worker's precheck like a verify, and \
                   it is never read as a verify green. A running one is shown as `precheck in \
                   flight` and keeps the worker from reading as idle; it does not hold a landing. \
                   With `\"precheck\": true` in `.claude/air.json`, a branch is batch-ready only \
                   with a green precheck at its head (`no-precheck` otherwise).",
        silent_break: false,
        action: "Under a lane, run the repo's precheck through `air record precheck --`. To have \
                 the lane cut only prechecked heads, set `\"precheck\": true` and retire any log \
                 file or \"checked at <sha>\" message the lane reads for it.",
    },
    SurfaceChange {
        id: "tree-readers",
        since: "2026-09-25 (owner)",
        headline: "`air status` names every process that is not a Claude Code session with its \
                   working directory in a fleet tree (`readers: <tree>: N (<exe> <age>, ...)`, \
                   `--json` `tree_readers`), `idle-without-claim` no longer fires for a worker \
                   whose tree has one, and `air land` warns, without refusing, about any in the \
                   main checkout before moving main.",
        silent_break: false,
        action: "A repo-side tree-readers check (processes by cwd before landing or before \
                 calling a worker idle) can be retired in favour of `air status`. Nothing is \
                 required.",
    },
    SurfaceChange {
        id: "batch-cut",
        since: "2026-09-25 (owner)",
        headline: "New `air batch cut`, run in the lane's worktree: the batch-ready set, oldest \
                   ready first (committer time of the listed head); a `git merge-tree` pre-check \
                   of each member against main and each earlier member; then `git merge main` and \
                   each clean member at its listed sha, judged by `git ls-files -u` and conflict \
                   markers. A conflicting member is dropped and named with the other side and the \
                   paths, and each drop is an event line. `--dry-run` merges nothing. It needs \
                   git 2.38 or later, and it neither verifies nor lands.",
        silent_break: false,
        action: "A lane that cuts by hand or with its own merge script can replace that with \
                 `air batch cut` followed by `air record verify -- <verify command>`. Start the \
                 lane's branch from main: commits already on it and not in main are carried into \
                 the batch and listed as `carried`.",
    },
    SurfaceChange {
        id: "installed-skills-follow-roles",
        since: "2026-09-25 (air-vuwx)",
        headline: "The installed skills agree with `.air/roles.md`. `air-phase-transitions` is no \
                   longer installed: its `awaiting_review` step, close-after-landing and \
                   re-verifying landing contradicted the protocol. `air-decomposition` drops \
                   per-worker queues and the `next` subcommand, which never existed. The \
                   CLAUDE.md that `air init` scaffolds no longer says the hand-over flow is the \
                   repo's own; it points at roles.md and keeps only what is the repo's.",
        silent_break: true,
        action: "`air install --write` removes `.claude/skills/air-phase-transitions/` (see \
                 `install-removes-retired-skills`). If this repo's CLAUDE.md restates a close or hand-over sequence, cut it to \
                 what roles.md leaves to the repo: the verify and precheck commands, worktree \
                 setup, shared resources and domain rules.",
    },
    SurfaceChange {
        id: "bd-budgets-generous-and-scaled",
        since: "2026-09-25 (air-8lj8)",
        headline: "bd calls that refuse a command on timeout are generous now: 60 s per bd process \
                   (was 10 s), plus 5 s per id for every call naming several ids, from one \
                   shared function. `air close <ids...>` gets the per-id allowance for the first \
                   time (it was a flat 10 s), and `air land`'s acceptance read goes from 10 s + \
                   2 s per id to 60 s + 5 s per id. The probe after a timed-out claim, and the \
                   id check in a triage pass, go from 5 s to 30 s. `air status` (8 s cap, under \
                   the 20 s MCP tool limit) and the Stop nudge (3 s, under the 5 s hook cap) are \
                   unchanged. A timeout now names the id count, the budget and the override.",
        silent_break: false,
        action: "If you exported `AIR_BD_TIMEOUT_MS` to get past the old 10 s, you can drop it: \
                 while set it replaces the WHOLE budget with one flat figure, per-id allowance \
                 included, so a large `air close` can hit it where the default would not.",
    },
    SurfaceChange {
        id: "mcp-tool-budgets",
        since: "2026-09-25 (air-se4n)",
        headline: "`air mcp` no longer cuts every tool off at 20 s. A tool whose command runs bd \
                   at the full bd budget gets one more of those than its command can spend, \
                   from the same function: close 2 x (60 s + 5 s per id), triage and handover \
                   2 x 65 s, release 3 x 65 s, claim 7 x 65 s, all capped 10 s under Claude \
                   Code's MCP_TOOL_TIMEOUT (default 300 s). Status, attention, holdings, inbox \
                   and capture stay at 20 s. A timed-out tool now kills its whole \
                   process group, so no bd process it started keeps running.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "names-carry-role",
        since: "2026-09-25 (owner, air-jc2p.4)",
        headline: "Names carry the role and the project. `air worker` with no name makes \
                   `worker-<N>` (was `w<N>`); `main`, `coordinator` and `lane` are refused as \
                   worker names. Every launch passes `claude --name <project>-<name>`, the same \
                   string as its tmux session, so `ListAgents` and `SendMessage` use it. \
                   `<project>` is `\"project\"` in `.claude/air.json` when set, else the beads \
                   prefix as before. A launch whose tmux session already exists is attached to \
                   when it was started in the same worktree, and refused, naming the other \
                   directory, when it was not.",
        silent_break: false,
        action: "Existing `w<N>` worktrees keep working as workers; nothing to rename. A repo \
                 whose project name collides with another fleet on the same machine sets \
                 `\"project\"` in `.claude/air.json`. Anything that sends to a worker's session \
                 by its old harness-made name uses `<project>-<name>` after a relaunch.",
    },
    SurfaceChange {
        id: "lane-lands",
        since: "2026-09-25 (owner ruling 2026-09-14, air-jc2p.2)",
        headline: "New `air lane [<name>]` starts the verification lane (worktree `lane` by \
                   default): a worker session with AIR_ROLE=lane and the worker deny list minus \
                   `air land`. `air land` is now refused to AIR_ROLE=coordinator as well as to \
                   workers; the lane and the owner's own shell may land. `air close` is refused \
                   to the lane as to a worker. Workers are denied `air lane`.",
        silent_break: true,
        action: "Start the lane with `air lane` (`air lane w4` keeps an existing worktree). A \
                 lane still running as `air worker <name>` is refused `air land` and is offered \
                 ready beads again until relaunched. A coordinator that landed must hand that to \
                 the lane; the owner's shell (no AIR_ROLE) can still land by hand.",
    },
    SurfaceChange {
        id: "coordinator-worktree",
        since: "2026-09-25 (owner rulings 2026-09-14 and 2026-09-25, air-jc2p.1)",
        headline: "`air coordinator` now creates or reuses `.claude/worktrees/coordinator` \
                   (branch `worktree-coordinator`) and always starts in the tmux session \
                   `<project>-coordinator`, attaching when it is already running. The \
                   coordinator's session is recorded as `coordinator`, not `main`. Its branch is \
                   batch-ready with no claimed bead once it has a commit main lacks, so its \
                   commits reach main in the lane's batch.",
        silent_break: true,
        action: "Start the coordinator with `air coordinator` and attach with `tmux attach -t \
                 <project>-coordinator`; it needs tmux. `.mcp.json` must be tracked so the \
                 channel server is present in the worktree. Commit coordinator prose in its \
                 worktree, not in the main checkout. Anything that looked up the coordinator's \
                 session or events as `main` reads `coordinator` from now on.",
    },
    SurfaceChange {
        id: "main-checkout-sessions",
        since: "2026-09-25 (air-jc2p.3)",
        headline: "`air status` prints `warning: <role> session <name> (pid N) runs in the main \
                   checkout` for each launched session whose `claude` process has its working \
                   directory there (`--json` `main_checkout_sessions`). The owner's own shell \
                   (no AIR_ROLE) is exempt, and nothing is refused.",
        silent_break: false,
        action: "Relaunch such a session through `air coordinator`, `air lane` or `air worker`, \
                 each of which runs in its own worktree.",
    },
    SurfaceChange {
        id: "air-dir-records",
        since: "2026-09-25 (owner, air-1qnp)",
        headline: "Air's generated records go in the main checkout's gitignored `.air/`. \
                   Session journals default to `.air/journal/` (`journal_dir` is now an \
                   override), and `air init` no longer scaffolds `docs/journal/` or writes \
                   `journal_dir`. New opt-in `\"digests\": true` in `.claude/air.json` makes the \
                   close gate require a digest in `.air/digests/`, checked for existence rather \
                   than git tracking. The worktree fence lets a worker write under those two \
                   directories. A repo with `digest_dir` keeps the tracked check unchanged; one \
                   with neither key still needs no digest.",
        silent_break: false,
        action: "Nothing required. To move journals out of your tree, drop `journal_dir` from \
                 `.claude/air.json` and move the files into `.air/journal/`; a repo that wants \
                 digests without committing them replaces `digest_dir` with `\"digests\": true`. \
                 Both directories live on the machine running the fleet and have no git \
                 history.",
    },
    SurfaceChange {
        id: "roles-rewritten-per-role",
        since: "2026-09-25 (owner, air-igcz)",
        headline: "`.air/roles.md` is rewritten as facts and refusals per role (every role, \
                   worker, verification lane, coordinator), at about 60% of its old length. \
                   Incident histories, dates, bead ids and removal conditions moved out \
                   (removal conditions are in `air audit`). Corrected: a landed bead whose \
                   acceptance clause names a file the merge did not change is a lookup that did \
                   not answer, not a wrong close; the lease gate refuses the lane as well as \
                   workers; journals and opt-in digests live under the main checkout's `.air/`. \
                   `docs/rules/adopting-air.md` is cut to install, the `.claude/air.json` keys \
                   and upgrading.",
        silent_break: false,
        action: "Nothing to run; `air install --write` refreshes `.air/roles.md`. Restart \
                 sessions to give them the new text. If the repo's CLAUDE.md quotes a roles.md \
                 sentence, check it still appears.",
    },
    SurfaceChange {
        id: "init-any-project",
        since: "2026-09-26 (owner, air-gn5o)",
        headline: "`air record` no longer flags a green `suspicious` for being under 2 s. It \
                   flags a green that printed nothing, or that ran in under a fifth of the time \
                   this worker's last green of the same command took. `air init` in a new repo \
                   proposes the verify command it finds (a Makefile's `verify` or `test`, \
                   `cargo test`, `npm test`) and writes the failing `make verify` placeholder \
                   only when it finds none; lets bd name the bead prefix after the directory; \
                   writes `\"project\"` as the directory name and `\"metis\"` as whether Metis \
                   is installed; and no longer writes `\"adopters\"` or runs `bd config set \
                   status.custom awaiting_review`.",
        silent_break: false,
        action: "Nothing required. A repo that read `suspicious` as \"under 2 s\" now sees it \
                 only on a drop against its own history.",
    },
    SurfaceChange {
        id: "print-writes-nothing",
        since: "2026-09-25 (an adopter's re-audit, air-rr98)",
        headline: "`--print` on `air worker`, `air lane` and `air coordinator` writes nothing. \
                   It used to rewrite `.air/roles.md` (and `.air/coordinator.md`), and with \
                   `--task` write `.air/tasks/<name>.md`; the printed line now names a task \
                   file that does not exist yet.",
        silent_break: false,
        action: "Refresh `.air/roles.md` with `air install --write`, not with a printed launch.",
    },
    SurfaceChange {
        id: "install-removes-retired-skills",
        since: "2026-09-25 (an adopter's re-audit, air-rr98)",
        headline: "`air install --write` removes each `air-*` skill directory Air once installed \
                   and no longer ships (today `air-phase-transitions`) and names each one. A \
                   skill whose name does not start with `air-` is never touched. The installed \
                   skills and `.air/roles.md` no longer name a skill Air does not install: \
                   roles.md now says `air-decomposition`.",
        silent_break: false,
        action: "Nothing to run beyond `air install --write`, then commit the removal.",
    },
    SurfaceChange {
        id: "verify-lane-is-a-switch",
        since: "2026-09-25 (air-rr98)",
        headline: "`verify_lane` in `.claude/air.json` is now `true` or absent. `true` puts the \
                   with-lane closing sequence in force; it no longer names the lane, which is \
                   the session `air lane` started. A string value still counts as `true`, and \
                   no code reads the key: `air batch cut`'s refusal in the main checkout names \
                   `air lane`'s worktree instead of the key's value.",
        silent_break: false,
        action: "Optional: change `\"verify_lane\": \"<name>\"` to `\"verify_lane\": true`. A \
                 lane whose worktree is not `lane` is started with `air lane <name>`.",
    },
    SurfaceChange {
        id: "launch-needs-install-committed",
        since: "2026-09-25 (owner, air-rr98)",
        headline: "`air worker` and `air lane` refuse, naming the files, while `air install`'s \
                   output (`.claude/settings.json`, `.mcp.json`, `.claude/skills/air-*`) has \
                   uncommitted changes in the main checkout. A new worktree gets only \
                   committed files, so such a session ran without hooks, channel or skills.",
        silent_break: false,
        action: "Commit those files on main after every `air install --write`.",
    },
    SurfaceChange {
        id: "coordinator-starts-the-fleet",
        since: "2026-09-25 (owner, air-jc2p.5)",
        headline: "`air coordinator` asks on its terminal \"Start the fleet (lane + N workers)? \
                   [Y/n]\" before it starts. Yes, or the new `air fleet up`, starts the lane and \
                   N workers (`\"workers\"` in `.claude/air.json`, default 3, 0 allowed) as \
                   `lane` and `worker-<N>`, each in its worktree and a detached tmux session, \
                   leaving one already running alone. Workers start with no task; the lane is \
                   told to start its loop. `--fleet` and `--no-fleet` answer in advance; with no \
                   terminal the answer is no. Workers and the lane are denied `air fleet`.",
        silent_break: false,
        action: "A script that runs `air coordinator` with a terminal passes `--no-fleet` to \
                 keep today's behaviour. A repo whose workers use other names (`w1`) gets \
                 `worker-<N>` sessions beside them from `air fleet up`.",
    },
    SurfaceChange {
        id: "bead-context-section",
        since: "2026-09-25 (owner, from an adopter)",
        headline: "The installed `air-decomposition` skill asks every bead to end with a \
                   `## Context` section naming the skills the worker invokes first, the files \
                   and doc sections to read first, and any doc the work must update; \
                   `.air/roles.md` tells the worker who claims it to do so.",
        silent_break: false,
        action: "A repo that added its own context section to its copy of the skill loses that \
                 edit on `air install --write`; keep project-specific skill lists in the repo's \
                 own intake guide and name them in the section.",
    },
    SurfaceChange {
        id: "every-decision-measured",
        since: "2026-09-25 (owner, air-hqj8)",
        headline: "`air audit` has a row for every decision Air writes and every timing budget \
                   it can hit, each with a `counts:` line naming what it counts; rows with no \
                   recorded removal condition say so as a defect. New event lines: \
                   `batch-cut / no-precheck` per branch the precheck kept out of a cut, \
                   `status / main-checkout-session` when `air status` prints that warning, and \
                   `mcp.tool / timeout` when an MCP tool's subprocess is killed. Renamed: \
                   `batch-cut / dropped` is now `batch-cut / drop`, and `air land`'s role \
                   refusal is `land / refuse-role` rather than `land / refuse`.",
        silent_break: true,
        action: "Only if something of the repo's reads `.air/events/`: match the two renamed \
                 decisions.",
    },
    SurfaceChange {
        id: "install-pin",
        since: "2026-09-25 (owner, air-4usc)",
        headline: "`air install --write --pin` copies the running binary to `.air/bin/air` and \
                   points the hooks and the `.mcp.json` channel at that copy by absolute path. \
                   The launchers then put `.air/bin` first on PATH in every session they start, \
                   and `air status` and `air doctor` name the pin and warn when the `air` on \
                   PATH has a different surface version. `--unpin` goes back to PATH. A pinned \
                   repo refuses a plain `air install --write` until `--pin` or `--unpin` is \
                   given.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "pin-delegation",
        since: "2026-09-25 (air-qyrm)",
        headline: "In a pinned repo, an `air` that is not the pin hands every command to the pin \
                   (`<main checkout>/.air/bin/air`) before running it, with the same arguments, \
                   so a bare `air` runs the pin whatever order a shell put PATH in; \
                   `air --version` there prints the pin's version. `air install` is the one \
                   command that runs where it was found, so re-pinning with a newer build works. \
                   Each hand-over writes one `pin / delegated` event line.",
        silent_break: false,
        action: "Only if the `air` on PATH is older than this release: install this one, or \
                 bare `air` in a pinned repo still runs the PATH binary.",
    },
    SurfaceChange {
        id: "lane-land-allow",
        since: "2026-09-25 (air-8cdh)",
        headline: "`air lane` (and `air fleet up`) put `permissions.allow: [\"Bash(air land *)\"]` \
                   on the lane's own `--settings`, plus the pinned binary's absolute path when \
                   the repo is pinned. In auto mode an allow rule resolves before the \
                   classifier, which had denied the lane's `air land` as \"[Modify Shared \
                   Resources]\". No other role gets it; a pass-through `--settings` allow list \
                   is now combined with Air's rather than replaced.",
        silent_break: false,
        action: "Restart a running lane through `air lane` or `air fleet up` to pick it up.",
    },
    SurfaceChange {
        id: "launch-startup-prompts",
        since: "2026-09-25 (air-oe9k)",
        headline: "Every launched session approves the `air` server from `.mcp.json` in its own \
                   `--settings` (`enabledMcpjsonServers`), so the \"New MCP server found\" \
                   question, whose default dropped the channel, is gone. `air coordinator` and \
                   `air fleet up` print once the prompts that remain: folder trust until the \
                   main checkout is trusted (answer yes; one yes covers every worktree) and the \
                   coordinator's development-channels warning.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "channel-every-session",
        since: "2026-09-26 (air-1vri)",
        headline: "The lane and every worker now load the Air channel, as the coordinator \
                   already did, and each session's `air mcp` pushes into it the messages Air \
                   addressed to that session (ledger table `deliveries`, one event line \
                   `channel.deliver / delivered` per message). So every session Air starts \
                   shows the development-channels warning; answer \"I am using this for local \
                   development\". Only the coordinator's and the owner's `air mcp` run the \
                   attention poll now; a worker's used to run it into a session with no \
                   channel.",
        silent_break: false,
        action: "Restart running sessions through `air fleet up`, `air worker` or `air lane` \
                 to attach the channel.",
    },
    SurfaceChange {
        id: "beads-ready-fanout",
        since: "2026-09-26 (air-dkm1)",
        headline: "When the claimable ready set gains a bead, the coordinator's channel tells \
                   every live, idle worker holding no claim \"beads are ready: <ids>\", once per \
                   change (one `fanout / beads-ready` event line). The lane and workers holding \
                   a claim are not told. It asks bd for the ready list at most once a minute. \
                   The `idle-without-claim` condition no longer ends \"prompt them\". \
                   `.air/roles.md` says what the message means.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "fresh-session-idle",
        since: "2026-09-26 (air-ludo)",
        headline: "A session is recorded idle at its start, not working: a worker launched with \
                   no task takes no turn, so `air status` called it `working since <launch>` for \
                   good. The first tool call makes it `running`; a start caused by compaction \
                   keeps its state. \"beads are ready\" now goes to every worker with a live \
                   session holding no claim, whatever its state, not only an idle one; the \
                   2026-09-26 trial's fleet reached nobody and stalled at `ready: 3`. \
                   `idle-without-claim` can now name a worker that never took a turn.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "batch-events-pushed",
        since: "2026-09-26 (air-1vri.2)",
        headline: "The lane is told each branch that becomes batch-ready, and each member \
                   worker is told its batch's result at once: `batch green ... Close them now` \
                   from `air record`, `batch red` with the exit and the kept output, `landed in \
                   main` from `air land`, and `dropped from batch` with the conflict from `air \
                   batch cut` (event lines `fanout / batch-ready` and `fanout / batch-result`). \
                   `air land` and a red batch's `air record` now end with the batch-ready set \
                   and `next: air batch cut`, or `nothing is batch-ready`; a green batch's ends \
                   with `next: air land --worker <lane>`, and `air land --json` carries \
                   `batch_ready`. `air status` prints `loops (24 h):` with the median wait from \
                   batch-ready to its batch verify and from batch green to close.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "fleet-stop",
        since: "2026-09-26 (air-1vri.1)",
        headline: "`air fleet stop [--reason <why>]` and `air fleet resume` (coordinator and \
                   owner only; a worker or the lane is refused) stop and resume all work with \
                   one command. Every other session is told through its channel. While \
                   stopped, `air claim`, `air batch cut` and `air land` refuse naming the stop \
                   and who set it, the ready fan-out and the Stop nudge are silent, and `air \
                   status` leads with `FLEET STOPPED`. Sessions are not killed, and a verify \
                   already running finishes and is recorded. New ledger table `fleet_stop`. \
                   `.air/roles.md` says what a stop means for each role.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "lease-free-told",
        since: "2026-09-26 (air-1vri.3)",
        headline: "When a lease is released (`air lease release`) or broken (`air lease \
                   break`), each worker that was refused it hears \"<lease> is free\" once, \
                   oldest want first (event line `fanout / lease-free`). Its want is cleared \
                   when the message is delivered or when it takes the lease.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "no-chatter",
        since: "2026-09-26 (air-uzh2)",
        headline: "roles.md no longer tells a worker to message the coordinator when it closes a \
                   bead; it messages only when blocked, needing a decision or finding work \
                   outside its bead, and then through `air capture`. The coordinator files and \
                   prioritises beads and does not message a worker to hand one out unless the \
                   owner asks. `air audit` prints worker-to-coordinator messages per closed bead.",
        silent_break: true,
        action: "If the repo's own prose tells workers to report each close, or the coordinator \
                 to assign beads by message, remove it.",
    },
    SurfaceChange {
        id: "main-moved-to-all",
        since: "2026-09-26 (air-1vri.4)",
        headline: "When `air land` moves main, the coordinator and every worker hear \"main \
                   moved to <sha>: landed <beads>; files changed: <paths>\" once per landing \
                   (event line `fanout / main-moved`). The lane that ran the landing is not \
                   told again. A member's copy also says its beads are in and may be closed, \
                   and replaces the separate `landed in main` message, which is gone. \
                   `.air/roles.md` says what it means for each role; it needs no reply.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "coordinator-capture-and-queue-empty",
        since: "2026-09-26 (air-1vri.5)",
        headline: "The coordinator hears \"capture from <worker>: <first line>\" once when a \
                   worker runs `air capture`, and \"the ready queue is empty: <n> worker(s) \
                   idle; epics with no open child: <ids or none>\" once per emptying while a \
                   worker holds no claim (event lines `fanout / capture` and `fanout / \
                   queue-empty`). `.air/roles.md` says what to do with each.",
        silent_break: false,
        action: "If the repo's own prose tells the coordinator to poll `air inbox` or the ready \
                 queue on a timer for these, it can rely on the notices instead.",
    },
    SurfaceChange {
        id: "coordinator-commits-land-with-no-bead",
        since: "2026-09-26 (0.4.4 live trial)",
        headline: "`air land --worker <lane>` lands a batch whose only non-merge commits are \
                   the coordinator's (its `coordinator` worktree branch) carrying no bead, as \
                   the batch-ready rule already took them. Any other commit with no `Bead:` \
                   trailer is still refused, and the lane is now told to have the member add \
                   a trailer and cut again rather than to `git commit --amend`.",
        silent_break: false,
        action: "",
    },
    SurfaceChange {
        id: "coordinator-does-not-implement",
        since: "2026-09-26 (0.4.4 live trial)",
        headline: "`.air/roles.md` tells the coordinator it does not implement: anything to \
                   build, fix or write, a helper script included, goes to a worker as a bead, \
                   and it commits in its own worktree only when no worker can make the change.",
        silent_break: false,
        action: "If the repo's own prose has the coordinator doing implementation work, move \
                 that to beads.",
    },
    SurfaceChange {
        id: "bd-pin-1-3-0",
        since: "2026-09-26 (owner ruling)",
        headline: "Air now expects bd 1.3.0, not 1.2.2: `air doctor` compares `bd --version` \
                   with 1.3.0 and reports 1.2.2 as off the pin. Known difference: bd 1.3.0 \
                   refuses `bd update <id> -s open -a \"\"` on a bead another actor holds \
                   in_progress, which is the reopen `air release` issues; a fix is pending.",
        silent_break: false,
        action: "`brew unpin beads && brew upgrade beads`, then `bd --version` should print \
                 1.3.0. The store migrates on first use. Pin again with `brew pin beads` if \
                 you pinned before.",
    },
    SurfaceChange {
        id: "bd-server",
        since: "2026-09-26 (owner ruling)",
        headline: "`air init --write` on a new project sets bd up in server mode: it starts a \
                   Dolt server on a free port in 3400..3900 (data in `.air/dolt/`, tmux \
                   session `<project>-dolt`), runs `bd init --server`, and keeps the port in \
                   `.beads/dolt-server.port`. For a project in server mode, `air bd-server up`, \
                   `air fleet up` and the launchers start that server when it does not answer, \
                   `air doctor` and `air status` print a `bd:` line with the mode, and the \
                   coordinator's channel restarts a server that stops and tells the \
                   coordinator once. A project whose bd is embedded is not changed.",
        silent_break: false,
        action: "",
    },
];

/// The commit this binary was built from (`build.rs`), `unknown` outside a checkout.
pub const BUILD: &str = env!("AIR_BUILD");

/// What this binary IS, for a reader who has to attribute a log line or a session to a build
/// (air-dwq5). Pure, so the probe asserts on the string rather than on a process.
///
/// The crate version alone cannot do it, and tonight is why. A round's worth of behaviour
/// changes shipped under `0.2.19` because lanes cut no release rows (air-mir, a deliberate
/// trade: the exact-count check moved to `make release`), so the installed binary and every
/// branch build report the same string while behaving differently. Measured 2026-09-06: the
/// installed `air` and a branch build both said `air 0.2.19` while emitting different
/// Stop-hook advice, one corrected by air-avj and one not.
///
/// That trade is not being reversed here. What was wrong is that Air already RECORDS the
/// distinguishing fact — `build.rs` embeds it, `installed.json` stores it, the install-lag
/// check reads it — and told nobody. This says it.
///
/// The surface version rides along because it is the number that actually moves when
/// behaviour changes mid-round, which is the question a reader is really asking.
pub fn version_line() -> String {
    format!(
        "air {} (built from {}, surface {})",
        env!("CARGO_PKG_VERSION"),
        BUILD,
        SURFACE_VERSION
    )
}

/// The same fact as JSON, for `air --version --json` and `air status --json` (air-dwq5).
/// `--version --json` used to print the plain string, which is the one shape a JSON reader
/// cannot parse.
pub fn version_json() -> serde_json::Value {
    serde_json::json!({
        "name": "air",
        "version": env!("CARGO_PKG_VERSION"),
        "built_from": BUILD,
        "surface_version": SURFACE_VERSION,
    })
}

/// What `.air/installed.json` records, so the diff has a baseline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Installed {
    /// The crate version that last wrote this file. It has been `0.0.1` in every build Air has
    /// ever produced, so it cannot tell two binaries apart; `built_from` is the field that can.
    pub air_version: String,
    /// The commit the binary that last wrote this file was built from (air-w9d). Empty on a
    /// file written before this existed, which reads as "cannot tell" and never as a mismatch.
    pub built_from: String,
    pub installed_at: String,
    /// Surface ids this repo has already been told about. What to PRINT.
    pub surface: Vec<String>,
    /// The monotonic surface version of the binary that last wrote this. What decides whether
    /// a write is allowed. `None` on a file written before air-w9d, which allows the write.
    pub surface_version: Option<u32>,
}

/// Changes this repo has not been told about yet. Pure, so the probe does not need a repo.
pub fn surface_diff(known: &[String]) -> Vec<&'static SurfaceChange> {
    SURFACE
        .iter()
        .filter(|c| !known.iter().any(|k| k == c.id))
        .collect()
}

/// Every release of Air, as `(crate version, surface version, notice count)`.
///
/// **This table is the line in the sand. Append one row per release; never edit a row.**
///
/// Owner, 2026-08-29: *"ensure that our whole process knows to bump the monotonic id every
/// time we cut a release... enforce a good release system, so that we draw those lines in the
/// sand more readily."*
///
/// Until that ruling Air had no release concept at all: `version = "0.0.1"` since the first
/// commit, no tags, and `Installed.air_version` recording a string that could not tell two
/// binaries apart while its own doc claimed it could. A surface version that moves on nobody's
/// authority is the same defect one level up.
///
/// **Releases are cut per round, not per notice** (owner, 2026-09-06, air-mir). A lane
/// appending a notice appends NO row; the coordinator appends one row at round end covering
/// every notice since the last. What that ruling moved is WHEN the count has to match, not
/// whether: the invariant — a surface notice never ships without a row, so `Installed` never
/// claims a version it cannot identify (air-w9d) — is unchanged.
///
/// So the check is split across two moments:
///
/// * `make verify` (`tests::a_release_row_matches_the_crate_and_the_surface`) asks only that
///   nothing went BACKWARDS ([`verify_rows_ok`]) and that the crate version matches the last
///   row. Notices beyond that row are the normal mid-round state.
/// * `make release` (`air release-check`, [`release_check`]) asks that the count and the crate
///   version agree with the last row exactly, and names the row to append when they do not.
///
/// The cost of asking at every verify was measured, not guessed: nineteen releases between
/// 17:22Z and 00:50Z on 2026-09-06, five release-row number collisions between lanes
/// re-numbered by coordinator message, and a tag/verify/install cycle of several minutes on
/// main for every landing that carried a notice.
///
/// Forgetting fails toward PERMITTING — the downgrade refusal quietly stops noticing — which
/// is the one direction a guard must not fail in, and is why the release half is a refusal and
/// not a comment.
///
/// Removal condition for the release-time check: when notices are generated from the release
/// rows rather than written by hand, at which point they cannot outrun them.
pub const RELEASES: &[(&str, u32, usize)] = &[
    // The surface as it stood before 2026-08-29: nine notices, no release ever cut.
    ("0.0.1", 1, 9),
    // 2026-08-29: the twelve notices of air-njb, and the forward-only install of air-w9d.
    ("0.1.0", 2, 21),
    // 2026-09-05: the `messages` table of air-srv; the ledger now holds message content.
    ("0.1.1", 3, 22),
    // 2026-09-05: green keyed by commit not (worker, sha), tree key by declaration, schema
    // v14 (air-7wf).
    ("0.2.0", 4, 23),
    // 2026-09-05: the task file of air-er0 (the prompt leaves the worker's command line).
    ("0.2.1", 5, 24),
    // 2026-09-05: `air land --worker` names the branch (air-09b).
    ("0.2.2", 6, 25),
    // 2026-09-05: the owner inbox goes (air-uef); the owner's queue is owner-labelled beads.
    ("0.2.3", 7, 26),
    // 2026-09-05: a signalled verify is no verdict (air-ppm).
    ("0.2.4", 8, 27),
    // 2026-09-05: the hand-over refusal names the held bead and skips the digest check with
    // nothing to declare (air-xbl).
    ("0.2.5", 9, 28),
    // 2026-09-05: env on the process, one merged --settings, UNENFORCED in status (air-9dg).
    ("0.2.6", 10, 29),
    // 2026-09-05: the gate accepts a bead carried by trailer; supersession has a path (air-60x).
    ("0.2.7", 11, 30),
    // 2026-09-05: `air land` refuses over a verify in flight; `--despite-inflight` is recorded
    // (air-1bm), schema v16; bd not answering about acceptance refuses before the merge
    // (air-bh4).
    ("0.2.8", 12, 31),
    // 2026-09-05: epics named apart from claimable work; `air claim` refuses one (air-f10).
    ("0.2.9", 13, 32),
    // 2026-09-05: bd_calls per event, one show per reconcile, SubagentStop is not a Stop
    // (air-bp0).
    ("0.2.10", 14, 33),
    // 2026-09-05: holdings tags name their tense; dirt is told from an edit (air-v7o).
    ("0.2.11", 15, 34),
    // 2026-09-05: Air creates, fills and removes worker worktrees (air-fdz).
    ("0.2.12", 16, 35),
    // 2026-09-05: `air install` reports a stale `bd prime` hook (air-b5k).
    ("0.2.13", 17, 36),
    // 2026-09-05: the digest refusal names the order that keeps the green (air-yol).
    ("0.2.14", 18, 37),
    // 2026-09-05: `AskUserQuestion` denied to workers and counted (air-bm3).
    ("0.2.15", 19, 38),
    // 2026-09-05: install and init refuse while .air/ is not ignored (air-6di).
    ("0.2.16", 20, 39),
    // 2026-09-05: `air status` lists batch-ready branches for the verify lane (air-80x.3).
    ("0.2.17", 21, 40),
    // 2026-09-05: the verification lane, gate acceptance (air-80x.1) and roles section (air-80x.6).
    ("0.2.18", 22, 41),
    // 2026-09-06: doctor and status name an install record that lags the binary (air-d61).
    ("0.2.19", 23, 42),
    // 2026-09-06: the second round of the day, cut per round rather than per notice (air-mir).
    // Sixteen surface changes across three lanes: the close gate asks the main a green was
    // recorded over, the Stop hook names `air handover` instead of restating a repair, the
    // digest must be tracked, a red run keeps its output, a build says which build it is, and
    // the overlap warning dates its holders.
    ("0.3.0", 24, 58),
    // 2026-09-06, second half of the round: an adopter's five reports after the log was first
    // written. handover-not-green stopped firing on successful closes, the refusal names which
    // of four not-green states it found, and a batch records the member a batch took rather
    // than where the branch is now.
    ("0.3.1", 25, 59),
    // 2026-09-06, the round's last landing: a discharged acceptance clause names the lookup
    // that discharged it, so the report is symmetric with the unreadable branch (air-rud0).
    ("0.3.2", 26, 60),
    // 2026-09-06, owner-ordered after the round: every session keeps a journal of what it hit,
    // for findings that imply no action and so have no home in a bead or a capture (air-3xww).
    ("0.3.3", 27, 61),
    // 2026-09-06, the second round of the day, four notices in one row per air-mir. The
    // adopter's fleet reported eight defects against a frozen binary and three were real: a
    // fast-forward that timed out after succeeding was reported and RECORDED as a refusal
    // (air-htmn); `air capture` gained `--file`, because a capture worth writing was refused
    // for its shape as a command line (air-45pw); and `idle-without-claim` stopped firing on a
    // session whose verify is in flight, where its remedy told a coordinator to interrupt a
    // batch (air-t6ap). Plus the undischarged-clause wording, which is a silent break: nothing
    // says CONTRADICTS or prescribes reopening any more, so anyone who acted on those firings
    // should re-read the beads (air-k6uh, air-jy99). And `air handover` now tells a worker its
    // branch was in a red batch, and what the landing gate would say about it (air-hpp8,
    // air-33rn).
    ("0.3.4", 28, 65),
    // 2026-09-07. Seventeen notices, twelve of them found by a sweep rather than written when
    // the change landed (air-wt1v): 66 landings over the day were read against SURFACE and one
    // in six of the changes that needed a notice had one. **Two of the twelve were named in the
    // 0.3.4 row's comment above and had no notice.** `air install` prints SURFACE and never
    // RELEASES, so that comment reached no adopter while reading exactly like coverage — a
    // prose sentence in the wrong table, which is worse than silence because it answers the
    // question a checker would ask. Nothing here substitutes for a notice; the notices are the
    // notices. The round-end sweep is now part of the coordinator's review.
    ("0.3.5", 29, 82),
    // 2026-09-25/26: the fleet protocol moved into Air (roles.md carries it; the adopting repo
    // keeps its commands), the lane lands and has its own launcher, every role has a worktree
    // and a role-named tmux session, the coordinator asks to start the fleet, and a repo can be
    // pinned to its own binary. Twenty-nine notices, each written with its change; the sweep
    // read every landing of the round against them.
    ("0.4.0", 30, 111),
    // 2026-09-26: 0.4.0 was never tagged: its live trial failed (pinning did not hold, the lane
    // could not land, the trial copy had no verify_lane, eleven start-up prompts). The fixes
    // added three notices: pin-delegation, lane-land-allow, launch-startup-prompts.
    ("0.4.1", 31, 114),
    // 2026-09-26: Air's notices to the fleet (ready beads, batch-ready and batch results, fleet
    // stop and resume, lease free) and workers no longer message the coordinator on close. 0.4.1
    // was not tagged: its second trial was stopped by the owner after scenario 1 to build these.
    ("0.4.2", 32, 120),
    // 2026-09-26: 0.4.2's trial stalled: a freshly launched worker was never idle, so the ready
    // fan-out reached no one (air-ludo). One notice: fresh-session-idle.
    ("0.4.3", 33, 121),
    // 2026-09-26: "main moved" to every session, and the coordinator told of captures and of
    // an empty ready queue (air-1vri.4, air-1vri.5). 0.4.3's trial was stopped by the owner to
    // add them.
    ("0.4.4", 34, 123),
];

/// Pure: may `make verify` pass with `len` notices against a last row that says `last`?
/// Yes while nothing went backwards (air-mir): a lane appends notices during a round and the
/// coordinator appends the row at round end, so `len > last` is the normal state mid-round.
pub fn verify_rows_ok(last: usize, len: usize) -> bool {
    len >= last
}

/// Pure: the release-time check (air-mir; owner ruling 2026-09-06, releases per round). The
/// invariant is unchanged, a notice never ships without a row; what moved is WHEN it is
/// asked: here, from `make release`, instead of on every verify. Nineteen releases in one
/// day and five row collisions between lanes was the cost of asking every time. Returns the
/// message naming the count and the row to append.
pub fn release_check(cargo_version: &str, surface_len: usize) -> Result<(), String> {
    let (version, surface, count) = RELEASES.last().copied().unwrap_or(("", 0, 0));
    if version != cargo_version {
        return Err(format!(
            "Cargo.toml is at {cargo_version} but RELEASES' last row is {version}: append a \
             row for {cargo_version} (or set Cargo.toml to {version})."
        ));
    }
    if surface_len != count {
        return Err(format!(
            "SURFACE has {surface_len} notices but RELEASES' last row ({version}, {surface}, \
             {count}) covers {count}: append (\"<next version>\", {}, {surface_len}) and set \
             Cargo.toml to the same version, then re-run.",
            surface.saturating_add(1)
        ));
    }
    Ok(())
}

/// `air release-check`: exit 2 with the row to append when the surface has outrun the
/// releases. Run by `make release`, never by `make verify`.
pub fn release_check_cmd() -> i32 {
    match release_check(env!("CARGO_PKG_VERSION"), SURFACE.len()) {
        Ok(()) => {
            println!(
                "release-check ok: {} notices, last row {:?}",
                SURFACE.len(),
                RELEASES.last()
            );
            0
        }
        Err(e) => {
            eprintln!("release-check: {e}");
            2
        }
    }
}

/// The surface's version: monotonic, and **derived from [`RELEASES`] so it cannot drift from
/// it**. Bumped by cutting a release, never on its own.
///
/// This follows the ledger's `user_version` precedent (11 -> 12) rather than inventing a
/// scheme. The two jobs stay separate, which is the point:
///
/// * the number answers **may I write** — a total order, immune to a branch-only notice id;
/// * the id set answers **what do I print** — [`surface_diff`], unchanged and good at it.
///
/// A set was the first attempt and it cannot do the first job: a set says "different", never
/// "behind", so a worker installing from its own branch would make a later main-built binary
/// look older than the repo.
pub const SURFACE_VERSION: u32 = match RELEASES.last() {
    Some(&(_, v, _)) => v,
    None => 0,
};

/// May a binary at `mine` write over a repo recorded at `theirs`? (air-w9d)
///
/// `None` recorded means a repo installed before this existed: allowed, and deliberately so.
/// The file already treats a missing record as "told about nothing", and refusing here would
/// lock out every repo running Air today, the adopter included.
pub fn may_install(mine: u32, theirs: Option<u32>) -> bool {
    theirs.is_none_or(|t| mine >= t)
}

/// Read `.air/installed.json`. A missing or unreadable file means "told about nothing",
/// which is the right answer for a repo installed before Air recorded this.
pub fn read_installed(air_dir: &Path) -> Installed {
    std::fs::read_to_string(air_dir.join("installed.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// The install record is older than the running binary (air-d61).
///
/// The downgrade refusal (air-w9d) guards one direction: an old binary may not write over a
/// newer record. Nothing stated the other: the adopter's `.air/installed.json` said 0.1.0 /
/// surface 2 while its hooks had run 0.2.18 for days (the ledger already at schema v16, the
/// installed `air-*` skills still telling workers to run a refused command), and `air doctor`
/// said nothing. A repo whose hooks run a binary newer than the one it installed is exactly
/// the repo that has not read its notices. Fact, not gate: printed, never refused.
///
/// Removal condition: when the hook itself runs `air install --write` on first sight of a
/// newer binary, so the record cannot lag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallLag {
    pub recorded_crate: String,
    pub recorded_surface: Option<u32>,
    pub running_crate: String,
    pub running_surface: u32,
    /// Notices the record has not seen: what `air install` would print.
    pub unread: usize,
}

/// A dotted version as numbers, so `0.2.18` is newer than `0.2.9`. Non-numeric parts read as
/// zero; an empty string has no parts and compares below everything.
fn version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .split('.')
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Pure: does the record lag `crate_version` / `surface_version`? `None` when there is no
/// record (told about nothing, the case air-w9d deliberately allows) or the record is at the
/// binary or newer. Compared as versions, not as strings, because the crate version is what a
/// record written before `surface_version` existed still carries.
pub fn lag_against(
    rec: &Installed,
    crate_version: &str,
    surface_version: u32,
) -> Option<InstallLag> {
    if rec.air_version.is_empty() && rec.surface_version.is_none() {
        return None;
    }
    let crate_older = !rec.air_version.is_empty()
        && version_parts(&rec.air_version) < version_parts(crate_version);
    let surface_older = rec.surface_version.is_some_and(|s| s < surface_version);
    (crate_older || surface_older).then(|| InstallLag {
        recorded_crate: rec.air_version.clone(),
        recorded_surface: rec.surface_version,
        running_crate: crate_version.to_string(),
        running_surface: surface_version,
        unread: surface_diff(&rec.surface).len(),
    })
}

/// The lag of the record in `air_dir` against THIS binary.
pub fn lag(air_dir: &Path) -> Option<InstallLag> {
    lag_against(
        &read_installed(air_dir),
        env!("CARGO_PKG_VERSION"),
        SURFACE_VERSION,
    )
}

/// The one line `air doctor` and `air status` print for it.
pub fn lag_line(l: &InstallLag) -> String {
    let surface = |s: Option<u32>| s.map_or("none".to_string(), |v| v.to_string());
    format!(
        "install record lags the binary: installed {} / surface {}, running {} / surface {}, \
         {} notice(s) unread; fix: air install --write",
        if l.recorded_crate.is_empty() {
            "?"
        } else {
            &l.recorded_crate
        },
        surface(l.recorded_surface),
        l.running_crate,
        l.running_surface,
        l.unread
    )
}

fn write_installed(air_dir: &Path, at: &str) -> Result<(), String> {
    // MERGE, never overwrite (air-w9d). Overwriting let a binary drop ids the repo had already
    // been told about, so a single install by a stale `air` erased the record that the
    // downgrade check reads — the check and the erasure were the same write.
    let mut surface: Vec<String> = read_installed(air_dir).surface;
    for c in SURFACE {
        if !surface.iter().any(|k| k == c.id) {
            surface.push(c.id.to_string());
        }
    }
    let v = Installed {
        air_version: env!("CARGO_PKG_VERSION").to_string(),
        built_from: BUILD.to_string(),
        installed_at: at.to_string(),
        surface,
        surface_version: Some(SURFACE_VERSION),
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

/// A repo's pin as `air status` and `air doctor` report it (air-4usc).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PinState {
    /// The copy the hooks, the channel and Air's sessions run.
    pub binary: PathBuf,
    /// Its surface version, asked of the copy itself; None when it does not answer.
    pub surface: Option<u32>,
    /// The first `air` on PATH outside the pin's directory: what a shell that Air did not
    /// start runs.
    pub path_binary: Option<PathBuf>,
    pub path_surface: Option<u32>,
}

/// A binary's surface version from `--version --json`, or from the plain `--version` line of
/// a binary older than air-dwq5.
pub fn surface_of(bin: &Path) -> Option<u32> {
    let out = std::process::Command::new(bin)
        .args(["--version", "--json"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
        return v
            .get("surface_version")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok());
    }
    let tail = text.split("surface ").nth(1)?;
    tail.trim_end_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .ok()
}

/// The pin under `air_dir`, with both surface versions, or None when the repo is not pinned.
pub fn pin_state(air_dir: &Path) -> Option<PinState> {
    let binary = pin_path(air_dir);
    if !binary.is_file() {
        return None;
    }
    let pin_dir = binary.parent().and_then(|d| d.canonicalize().ok());
    let path_binary = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .filter(|d| d.canonicalize().ok() != pin_dir)
            .map(|d| d.join("air"))
            .find(|p| p.is_file())
    });
    Some(PinState {
        surface: surface_of(&binary),
        path_surface: path_binary.as_deref().and_then(surface_of),
        path_binary,
        binary,
    })
}

/// The pin's line, and a second when the pin and the PATH binary differ in surface version.
pub fn pin_lines(p: &PinState) -> Vec<String> {
    let v = |s: Option<u32>| s.map_or_else(|| "unknown".to_string(), |n| n.to_string());
    let mut out = vec![format!(
        "pinned: hooks, the channel and Air's sessions run {} (surface {})",
        p.binary.display(),
        v(p.surface)
    )];
    if let Some(path) = &p.path_binary
        && p.path_surface != p.surface
    {
        out.push(format!(
            "PIN DIFFERS from PATH: `air` on PATH is {} (surface {}); a shell Air did not start \
             runs that one, not the pin",
            path.display(),
            v(p.path_surface)
        ));
    }
    out
}

/// The `air` that PATH resolves to, if any.
fn air_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join("air"))
        .find(|p| p.is_file())
}

/// What `air install` does about the pin (air-4usc). `Keep` is no flag: a repo with no pin
/// stays on PATH, and a pinned repo refuses `--write` until one of the other two is said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinArg {
    Keep,
    Pin,
    Unpin,
}

/// Copy `from` to `to` by way of a temporary file and a rename, so a hook that is running the
/// old copy at that moment keeps its file and the next one gets the new one whole. A no-op
/// when they are already the same file (re-pinning from the pin itself).
fn copy_binary(from: &Path, to: &Path) -> Result<(), String> {
    if let (Ok(a), Ok(b)) = (from.canonicalize(), to.canonicalize())
        && a == b
    {
        return Ok(());
    }
    let dir = to.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".air.{}.tmp", std::process::id()));
    std::fs::copy(from, &tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, to).map_err(|e| format!("{}: {e}", to.display()))
}

#[derive(Debug, serde::Serialize)]
struct Plan {
    repo: PathBuf,
    binary: PathBuf,
    /// The copy the hooks and the channel run after this install, or None for PATH (air-4usc).
    pin: Option<PathBuf>,
    /// A pin was there before this run.
    pinned_before: bool,
    path_binary: Option<PathBuf>,
    binary_ok: bool,
    settings_path: PathBuf,
    settings_changed: bool,
    mcp_path: PathBuf,
    mcp_changed: bool,
    air_dir: PathBuf,
    skills_dir: PathBuf,
    /// Retired `air-*` skill directories present here, which `--write` removes (air-rr98).
    retired_skills: Vec<String>,
    gitignore_has_air: bool,
    /// Air's own surface changes this repo has not been told about (air-6g1). Empty on a
    /// first install: nothing has moved under a repo that never had Air.
    surface_diff: Vec<&'static SurfaceChange>,
    /// Hook entries that contradict Air and that the merge leaves in place (air-b5k).
    stale_hooks: Vec<StaleHook>,
    /// False when this binary's surface version is BELOW the one recorded here: a downgrade.
    forward: bool,
    /// The surface version recorded here, for the refusal's message.
    recorded_version: Option<u32>,
    /// What the binary that last installed here was built from, for the refusal's message.
    recorded_build: String,
    previously_installed: bool,
    written: bool,
}

/// A hook entry in the repo's settings that contradicts Air and that `air install` will not
/// remove (air-b5k). Reported on every install until it is gone, because a hand edit in an
/// adoption walkthrough is the step that gets skipped or done wrong once and never revisited,
/// and nothing else reports its state afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StaleHook {
    pub event: String,
    pub command: String,
    pub why: &'static str,
}

/// Why a `bd prime` hook is stale under Air, in the voice of the surface notices.
pub const BD_PRIME_WHY: &str = "`bd prime` injects a command reference telling agents to run \
`bd update --claim` and `bd create`, both of which Air denies; agents get instructions that \
contradict their deny list, and the failure looks like the agent being wrong. `air init` skips \
it (`bd init --skip-agents --skip-hooks`); a repo that adopted Air with it in place keeps it, \
because `air install` merges and never removes another tool's hook. Delete the entry.";

/// Every hook command in `settings` that runs `bd prime`. Pure over the JSON, so the probe
/// runs both directions without a repo. Only `bd prime` is stale today; a second stale
/// command is a second arm here, not a second scan.
pub fn stale_hooks(settings: &Value) -> Vec<StaleHook> {
    let mut out = Vec::new();
    let Some(hooks) = settings.get("hooks").and_then(Value::as_object) else {
        return out;
    };
    for (event, groups) in hooks {
        for group in groups.as_array().into_iter().flatten() {
            for h in group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(cmd) = h.get("command").and_then(Value::as_str) else {
                    continue;
                };
                let mut words = cmd.split_whitespace();
                let program = words.next().unwrap_or("");
                let is_bd = program == "bd" || program.ends_with("/bd");
                if is_bd && words.next() == Some("prime") {
                    out.push(StaleHook {
                        event: event.clone(),
                        command: cmd.to_string(),
                        why: BD_PRIME_WHY,
                    });
                }
            }
        }
    }
    out
}

/// The stale-hook block of the install report; empty when there is nothing to say.
pub fn render_stale(stale: &[StaleHook]) -> String {
    let mut s = String::new();
    for h in stale {
        s.push_str(&format!(
            "\nSTALE HOOK: {} runs `{}`, which contradicts Air\n        do: {}\n        since 2026-09-05 (air-b5k)\n",
            h.event, h.command, h.why
        ));
    }
    s
}

/// Would wiring the hooks change anything? False means Air is already installed here.
fn plan_settings_changed(before: &Value, after: &Value) -> bool {
    before != after
}

pub fn run(repo: &Path, write: bool, json: bool, pin_arg: PinArg) -> i32 {
    let repo = match repo.canonicalize() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("air install: {}: {e}", repo.display());
            return 1;
        }
    };
    let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("air"));
    let path_binary = air_on_path();
    let settings_path = repo.join(".claude/settings.json");
    let mcp_path = repo.join(".mcp.json");
    let air_dir = repo.join(".air");
    let pin_file = pin_path(&air_dir);
    let pinned_before = pin_file.is_file();
    // With `--pin` the hooks call the copy by absolute path, so the binary they run IS this
    // one whatever PATH says, and the PATH check below has nothing left to protect (air-4usc).
    let pin = match pin_arg {
        PinArg::Pin => Some(pin_file.clone()),
        PinArg::Unpin => None,
        PinArg::Keep => pinned_before.then(|| pin_file.clone()),
    };
    let binary_ok = pin_arg == PinArg::Pin
        || match (&path_binary, binary.canonicalize()) {
            (Some(p), Ok(me)) => p.canonicalize().map(|p| p == me).unwrap_or(false),
            _ => false,
        };
    let skills_dir = repo.join(".claude/skills");
    let retired_skills = retired_present(
        &std::fs::read_dir(&skills_dir)
            .map(|d| {
                d.filter_map(Result::ok)
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    );

    let before_settings = match read_json(&settings_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air install: {e}");
            return 1;
        }
    };
    let after_settings = merge_hooks_with(before_settings.clone(), &hook_command(pin.as_deref()));
    // What the merge leaves in place and should not (air-b5k). Read off the merged value so
    // the report describes the file as it will be after `--write`.
    let stale = stale_hooks(&after_settings);
    let before_mcp = match read_json(&mcp_path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("air install: {e}");
            return 1;
        }
    };
    let after_mcp = merge_mcp_with(
        before_mcp.clone(),
        &pin.as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "air".into()),
    );
    // git's answer, not a scan of one file: a nested `.gitignore`, `.git/info/exclude` or a
    // later `!.air` line all change it, and the ledger's content is what is at stake.
    let gitignore_has_air = air_ignored(&repo);

    // "Already installed" means the hooks are wired or `.air/roles.md` is there. Without
    // that, this is a first install and nothing has changed under anyone.
    let previously_installed = !plan_settings_changed(&before_settings, &after_settings)
        || air_dir.join("roles.md").exists();
    let recorded = read_installed(&air_dir);
    let surface_diff = if previously_installed {
        surface_diff(&recorded.surface)
    } else {
        Vec::new()
    };
    // May this binary write here at all? A total order, so a branch-only notice id cannot
    // make a main-built binary look older than it is (air-w9d).
    let forward = may_install(SURFACE_VERSION, recorded.surface_version);

    let mut plan = Plan {
        repo: repo.clone(),
        binary,
        pin: pin.clone(),
        pinned_before,
        path_binary,
        binary_ok,
        settings_path: settings_path.clone(),
        settings_changed: before_settings != after_settings,
        mcp_path: mcp_path.clone(),
        mcp_changed: before_mcp != after_mcp,
        air_dir: air_dir.clone(),
        skills_dir: skills_dir.clone(),
        retired_skills,
        gitignore_has_air,
        surface_diff,
        stale_hooks: stale,
        forward,
        recorded_version: recorded.surface_version,
        recorded_build: recorded.built_from.clone(),
        previously_installed,
        written: false,
    };

    if write {
        // Upgrades only (owner, 2026-08-29). A binary whose surface version is below the one
        // recorded here is older than the air that last installed, and writing would both
        // under-report the upgrade and, before the merge in `write_installed`, erase the
        // record of it.
        if !plan.forward {
            eprintln!(
                "{}",
                [
                    "air install: refusing to write: this repo was installed by a NEWER air."
                        .to_string(),
                    format!(
                        "  surface version here: {}   this binary: {SURFACE_VERSION}",
                        plan.recorded_version
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "none".into())
                    ),
                    format!(
                        "  build here: {}   this binary: {BUILD}",
                        if plan.recorded_build.is_empty() {
                            "not recorded (installed before air-w9d)"
                        } else {
                            &plan.recorded_build
                        }
                    ),
                    "  Air installs forward only. Install the newer air and re-run: cargo install --path crates/cli, from a checkout at or above that surface version."
                        .to_string(),
                ]
                .join("\n")
            );
            return 2;
        }
        if pin_arg == PinArg::Keep && pinned_before {
            eprintln!(
                "air install: refusing to write: this repo is pinned to {}, and its hooks and \
                 channel run that copy. Re-run with --pin to pin this binary in its place, or \
                 --unpin to go back to the `air` on PATH.",
                pin_file.display()
            );
            return 2;
        }
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
        if let Some(why) = ignore_refusal(plan.gitignore_has_air) {
            eprintln!("air install: refusing to write: {why}");
            return 2;
        }
        let steps: Result<(), String> = (|| {
            // The copy first, so no hook is ever written pointing at a file that is not there.
            if pin_arg == PinArg::Pin {
                copy_binary(&plan.binary, &pin_file)?;
            }
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
            for name in &plan.retired_skills {
                let dir = skills_dir.join(name);
                std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                // air-hqj8: counted by `retired-skill-removal`.
                if let Ok((ledger, me)) = super::open(&repo) {
                    super::log_event(
                        &ledger,
                        &me,
                        super::decisions::INSTALL_RETIRE_SKILL,
                        &serde_json::json!({"skill": name}),
                        "removed a skill Air no longer ships",
                        "1 skill",
                    );
                }
            }
            // After the hooks stop naming it.
            if pin_arg == PinArg::Unpin && pinned_before {
                std::fs::remove_file(&pin_file)
                    .map_err(|e| format!("{}: {e}", pin_file.display()))?;
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
            if pin_arg == PinArg::Pin {
                "pinned, so PATH does not matter"
            } else if plan.binary_ok {
                "is the `air` on PATH"
            } else {
                "NOT the `air` on PATH; fix before --write"
            }
        ));
        match (&plan.pin, pin_arg) {
            (Some(p), PinArg::Pin) => s.push_str(&format!(
                "pin:      {} {} (hooks and the channel run this copy)\n",
                if plan.written { "copied to" } else { "will copy to" },
                p.display()
            )),
            (Some(p), _) => s.push_str(&format!(
                "pin:      {} (hooks and the channel run this copy; --write needs --pin or --unpin)\n",
                p.display()
            )),
            (None, PinArg::Unpin) if plan.pinned_before => s.push_str(&format!(
                "pin:      {} {} (hooks and the channel go back to the `air` on PATH)\n",
                if plan.written { "removed" } else { "will remove" },
                pin_file.display()
            )),
            _ => {}
        }
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
        for name in &plan.retired_skills {
            s.push_str(&format!(
                "skills:   {} {} (Air no longer ships it)\n",
                if plan.written {
                    "removed"
                } else {
                    "will remove"
                },
                plan.skills_dir.join(name).display()
            ));
        }
        if let Some(why) = ignore_refusal(plan.gitignore_has_air) {
            s.push_str(&format!("REFUSAL:  {why}\n"));
        }
        s.push_str(if plan.written {
            "written.\n"
        } else {
            "dry run; re-run with --write to apply.\n"
        });
        if !plan.forward {
            s.push_str(&format!(
                "\nDOWNGRADE: this repo is at surface version {}, this binary is at {SURFACE_VERSION}. \
                 `--write` will refuse: Air installs forward only (air-w9d).\n",
                plan.recorded_version
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "none".into())
            ));
        }
        s.push_str(&render_surface(&plan.surface_diff, plan.written));
        s.push_str(&render_stale(&plan.stale_hooks));
        s
    });
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Collapse every run of whitespace to one space, so a pin can quote a sentence the way
    /// it reads rather than the way it happens to wrap (air-ahl). Two pins this round were
    /// written against a phrase that crossed a line break and failed on the break.
    fn flat(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

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
        // Every pin reads through `flat`, because roles.md is hard-wrapped and a pin that spans
        // a wrap fails on the wrap rather than on the rule. Each pin is a fact an agent acts
        // on; the incident behind it stays out of here and out of roles.md.
        let roles = flat(ROLES_MD);
        let has = |s: &str| roles.contains(&flat(s));
        // Run-to-completion is scoped to a session that HAS work; starting one is not being
        // given any. Both halves, because dropping either reverses the rule (air-7q5).
        assert!(has("Once you have work, finishing a bead is not a stop."));
        assert!(has("Starting a session is not being given work."));
        // 2026-09-26: SendMessage is for what Air does not already carry (air-uzh2, air-1vri).
        assert!(has("reach one worker with `SendMessage`"));
        // The heartbeat is the failsafe, and roles.md promises no `stuck` condition (air-12k).
        assert!(has("the heartbeat is the failsafe"));
        assert!(!has("the channel (stuck,"));
        // The protocol is Air's (owner, 2026-09-25) and the lane lands (air-jc2p.2). `--all` is
        // absent: under a lane the unit is the batch.
        assert!(has("the fleet's whole protocol"));
        assert!(has(
            "Landing is the lane's, not a worker's or the coordinator's"
        ));
        assert!(has("`air land --worker <lane>`"));
        assert!(!has("`air land --all`"));
        // A close needs no message, and the coordinator hands out no beads by message; the
        // signal-on-close rule (air-03w) is gone on purpose (air-uzh2).
        assert!(!has("Signal the coordinator when you close a bead"));
        assert!(has("A close needs no message."));
        assert!(has("then use `air capture`"));
        assert!(has(
            "Do not message a worker to hand it a bead unless the owner asks you to."
        ));
        assert!(!has("Give an idle worker its work with `SendMessage`"));
        // bd's per-type required sections (air-8zz).
        assert!(has(
            "bug `## Steps to Reproduce` + `## Acceptance Criteria`"
        ));
        assert!(has("epic `## Success Criteria`"));
        // A worker closes with proof; `awaiting_review` is never an instruction (air-8zu).
        assert!(
            !has("`bd update <id> -s awaiting_review`"),
            "roles.md must not prescribe a bead-status step"
        );
        assert!(has("You close your own bead, with proof"));
        assert!(has("You need not merge main to stay batch-ready"));
        assert!(has("commit forward and never amend"));
        // The worktree fence is Air's hook, not the harness's; the harness claim stays absent
        // because it would promise a block that is not there (air-8gj).
        assert!(has(
            "resolved path leaves your worktree is denied by Air's PreToolUse hook"
        ));
        assert!(!has("the main checkout is blocked natively"));
        // A `digest_dir` digest is tracked, and its route past a cut batch (air-ahl).
        assert!(has("It also has to be tracked by git there"));
        assert!(has("commit the digest without a `Bead:` trailer"));
        // Where a bead came from is a declared field, and the count refuses nothing (air-g5o).
        assert!(has("initiative: <CODE>"));
        assert!(has("It is a count and there is no refusal attached to it"));
        // The coordinator's reading is delegated by default (air-zth).
        assert!(has(
            "Your context is the channel the owner and every worker reach"
        ));
        // Air's messages and the fleet stop (air-1vri): each role hears them and knows what to
        // do, and the coordinator reaches the fleet with one command to Air.
        assert!(has("`beads are ready: <ids>` arrives when"));
        assert!(has("`batch-ready: <worker> at <sha> (<beads>)`"));
        assert!(has(
            "`fleet stop from the coordinator` means all work stops"
        ));
        assert!(has("run `air fleet stop --reason \"<why>\"`"));
        assert!(has(
            "one command to Air, not a `SendMessage` to each session"
        ));
        assert!(has("On `fleet stop`, let a verify already running finish"));
        // Naming reserves nothing, and the alternative is named beside it (air-u3l7).
        assert!(has("Naming a bead at a worker reserves nothing"));
        assert!(has("Put the craft notes on the bead, not in the message"));
        assert!(has("bd comment <id> --file <notes>"));
        // One owner queue, and it is beads (air-uef).
        assert!(has(
            "Every capture is triaged into a bead or dropped with a reason"
        ));
        assert!(has(
            "labelled `owner` with the coordinator's recommendation"
        ));
        assert!(!has("inbox --owner"), "the owner inbox is gone (air-uef)");
        assert!(!has("--for owner"), "the owner audience is gone (air-uef)");
        assert!(!has("owner decision waiting"));
        // Decomposition is a duty, not a property of the queue (air-84u).
        assert!(has("When an epic has no open child, decompose it"));
        assert!(has("the `air-decomposition` skill"));
        assert!(has("The reading may be delegated to a background agent"));
        assert!(!has("(epics decomposed;"));
        // A journal-only branch lands without a trailer; a mixed one does not (air-kexg).
        assert!(has(
            "a branch whose only commits are journal entries lands without one"
        ));
        assert!(has(
            "A branch that mixes them with anything else needs a trailer"
        ));
        // Main moving costs landability, never a close (air-9ij); landing does not re-verify,
        // which holds only for a verify that reads the tree (air-odv).
        assert!(has("main moving no longer retracts a close"));
        assert!(has("Air's landing does not re-verify"));
        assert!(has("reads the tree alone and not git history"));
        // The verification lane, under Worker, as facts and refusals. It names no cadence and
        // no worker: the lane cuts when branches are ready (air-80x.6).
        let lane = ROLES_MD
            .split("### Verification lane")
            .nth(1)
            .and_then(|s| s.split("\n## ").next());
        assert!(
            lane.is_some(),
            "roles.md has a Verification lane section under Worker"
        );
        let lane = lane.unwrap();
        let flat_lane = flat(lane);
        assert!(flat_lane.contains("A lane is a worker session like any other"));
        // The permission names the condition it assumes (air-4noi).
        assert!(flat_lane.contains("it may hold one **if its worktree survives the cut**"));
        // The loop is Air's; the batch comes from `air status`, never a relayed sha.
        assert!(flat_lane.contains("The lane's loop."));
        assert!(flat_lane.contains("never a sha from a message"));
        assert!(flat_lane.contains("Run `air batch cut` in your worktree"));
        assert!(flat_lane.contains(
            "The close gate accepts a green at a verified commit that contains `main` and every \
             commit carrying the bead's trailer"
        ));
        // Batch-ready does not require main (owner, 2026-09-25).
        assert!(flat_lane.contains("Being behind `main` does not take it out."));
        assert!(
            lane.lines().count() < 40,
            "under 40 lines: {}",
            lane.lines().count()
        );
        for word in ["cadence", "every N", "minutes", "hourly", "daily"] {
            // "nothing about cadence" is allowed once, as the statement that it is absent.
            let hits = lane.matches(word).count();
            assert!(
                hits <= usize::from(word == "cadence"),
                "lane section names a cadence: {word}"
            );
        }
        for name in ["worktree-verify", "air-verify", "verify lane is `"] {
            assert!(!lane.contains(name), "lane section names a worker: {name}");
        }
        assert!(ROLES_MD.find("### Verification lane") > ROLES_MD.find("## Worker"));
        assert!(ROLES_MD.find("### Verification lane") < ROLES_MD.find("## Coordinator"));
    }

    /// The verify-time half of the release line in the sand (owner, 2026-08-29; narrowed to
    /// this half by air-mir, 2026-09-06). What it still holds:
    ///
    /// * a notice REMOVED, or a row edited to say less, stops matching — rows are appended;
    /// * bump the crate version and the last row stops matching, so the surface version has
    ///   to be decided rather than drift.
    ///
    /// What moved to `air release-check`: notices beyond the last row's count. Mid-round that
    /// is the normal state, not a defect, since a lane appends a notice and no row.
    #[test]
    fn a_release_row_matches_the_crate_and_the_surface() {
        let (version, surface, count) = RELEASES.last().copied().unwrap_or(("", 0, 0));
        assert_eq!(
            version,
            env!("CARGO_PKG_VERSION"),
            "Cargo.toml is at {} but RELEASES' last row is {version}. Cutting a release means \
             appending a row here; see CLAUDE.md \"Releases\".",
            env!("CARGO_PKG_VERSION")
        );
        // air-mir: notices beyond the last row are allowed here and refused at release time
        // (`release_check`, run by `make release`). A count going BACKWARDS is still a lie.
        assert_eq!(SURFACE_VERSION, surface);
        assert!(
            verify_rows_ok(count, SURFACE.len()),
            "SURFACE has {} notices but RELEASES' last row says {count}: a notice was removed \
             or a row edited; rows are appended, never edited.",
            SURFACE.len()
        );
        // Monotonic in both machine-read columns, so `may_install` compares a total order
        // rather than an assumption, and no row may be edited to say less than the one before.
        assert!(
            RELEASES.windows(2).all(|w| match w {
                [(_, v1, c1), (_, v2, c2)] => v2 >= v1 && c2 >= c1,
                _ => true,
            }),
            "RELEASES must never decrease: append rows, never edit them"
        );
    }

    /// air-g5o: the split paragraph the coordinator's session carries when Metis is attached
    /// states the boundary Metis's own text denies. Pinned because the failure it prevents is
    /// the paragraph going missing and the coordinator taking a plugin's word for where the
    /// plan lives: Metis's instructions say it is the system of record, and here it is not.
    ///
    /// Each half is asserted separately. "Tasks are beads" without "decisions are dated
    /// entries" leaves the decision log to Metis; either without the declared field leaves
    /// `air status`'s count with nothing to read.
    #[test]
    fn the_metis_split_states_the_boundary() {
        assert!(METIS_SPLIT.contains("vision and initiatives"));
        assert!(METIS_SPLIT.contains("**Tasks are beads**"));
        assert!(METIS_SPLIT.contains("**Decisions are dated entries in `docs/`.**"));
        assert!(METIS_SPLIT.contains("initiative: <CODE>"));
        // It must say the count is not a gate, in the same breath as naming the count. A
        // session told about a number and not about its force will treat it as one.
        assert!(METIS_SPLIT.contains("It is a count, not a gate"));
        // And it must NOT reproduce Metis's claim, which is what it exists to contradict.
        assert!(
            !METIS_SPLIT.contains("system of record for tasks"),
            "the split must not restate the claim it corrects"
        );
    }

    /// air-b5k: the merge leaves a `bd prime` hook in place, so the report names it.
    #[test]
    fn a_bd_prime_hook_is_reported_and_air_hooks_are_not() {
        let with = json!({"hooks": {"SessionStart": [{"hooks": [
            {"type": "command", "command": "bd prime --hook-json"}]}]}});
        let after = merge_hooks(with);
        let stale = stale_hooks(&after);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].event, "SessionStart");
        assert_eq!(stale[0].command, "bd prime --hook-json");
        let text = render_stale(&stale);
        assert!(
            text.contains("STALE HOOK: SessionStart runs `bd prime --hook-json`"),
            "{text}"
        );
        assert!(text.contains("do: `bd prime` injects"), "{text}");
        // Air's own hooks, and a bd hook that is not prime, are not stale.
        let clean = merge_hooks(json!({"hooks": {"Stop": [{"hooks": [
            {"type": "command", "command": "bd ready --json"}]}]}}));
        assert!(stale_hooks(&clean).is_empty());
        assert_eq!(render_stale(&[]), "");
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
        // Read from the rule, not copied beside it (air-jc0): this assertion used to hold a
        // second copy of the matcher and went red when the matcher legitimately changed.
        let want = hook_entries()
            .into_iter()
            .find(|(e, _)| *e == "PreToolUse")
            .and_then(|(_, m)| m)
            .unwrap();
        assert_eq!(pre[1]["matcher"], want);
        // air-q07: a stale matcher on OUR entry is updated in place, not left alone. A repo
        // installed before the matcher widened must get the new one by re-running install.
        let stale = json!({"hooks": {"PreToolUse": [
            {"matcher": "Edit", "hooks": [{"type": "command", "command": "air hook"}]}
        ]}});
        let fixed = merge_hooks(stale);
        let pre = fixed["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1, "no duplicate entry: {pre:?}");
        assert_eq!(pre[0]["matcher"], want, "the matcher must be refreshed");
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
        assert_eq!(SKILLS.len(), 2);
        assert_eq!(SKILLS[0].0, "air-do-less");
        assert!(SKILLS.iter().all(|(_, text)| text.contains("Provenance")));
    }

    #[test]
    fn merge_mcp_adds_air_once_and_keeps_user_servers() {
        let v = merge_mcp_with(json!({"mcpServers": {"other": {"command": "x"}}}), "air");
        assert_eq!(v["mcpServers"]["air"]["args"][0], "mcp");
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        let custom = json!({"mcpServers": {"air": {"command": "/opt/air", "args": ["mcp", "-v"]}}});
        assert_eq!(merge_mcp_with(custom.clone(), "air"), custom);
        assert!(merge_mcp_with(json!("garbage"), "air")["mcpServers"]["air"].is_object());
    }

    /// air-4usc: `--pin` and `--unpin` move an installed repo's hooks and channel between the
    /// copy and PATH, in both directions, touching nobody else's entries.
    #[test]
    fn pin_rewrites_our_hooks_and_channel_and_unpin_restores_them() {
        let pin = Path::new("/r/my repo/.air/bin/air");
        let cmd = hook_command(Some(pin));
        assert_eq!(cmd, "'/r/my repo/.air/bin/air' hook");
        let mine =
            json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "mine"}]}]}});
        let unpinned = merge_hooks(mine);
        let pinned = merge_hooks_with(unpinned.clone(), &cmd);
        let cmds: Vec<&str> = pinned["hooks"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|g| g.as_array().unwrap())
            .flat_map(|g| g["hooks"].as_array().unwrap())
            .filter_map(|h| h["command"].as_str())
            .collect();
        assert!(cmds.contains(&"mine"), "{cmds:?}");
        assert!(!cmds.contains(&"air hook"), "{cmds:?}");
        assert_eq!(
            cmds.iter().filter(|c| **c == cmd).count(),
            hook_entries().len()
        );
        assert_eq!(merge_hooks(pinned), unpinned);
        let mcp = merge_mcp_with(json!({}), "/r/.air/bin/air");
        assert_eq!(mcp["mcpServers"]["air"]["command"], "/r/.air/bin/air");
        assert_eq!(
            merge_mcp_with(mcp, "air")["mcpServers"]["air"]["command"],
            "air"
        );
    }
}
