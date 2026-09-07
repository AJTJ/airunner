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
//! what the hooks resolve to).

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

fn air_hook() -> Value {
    json!({"type": "command", "command": "air hook", "timeout": HOOK_TIMEOUT_SECS})
}

fn is_ours(h: &Value) -> bool {
    h.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c == "air hook" || c.ends_with("/air hook"))
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
    let after_mcp = merge_mcp(before_mcp.clone());
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
        // air-7q5: run-to-completion is scoped to a session that HAS work; starting one is not
        // being given any. Both halves are pinned, because dropping either reverses the rule.
        assert!(ROLES_MD.contains("Once you have work, finishing a bead is not a stop."));
        assert!(ROLES_MD.contains("Starting a session is not being given work."));
        assert!(ROLES_MD.contains("Workers are reached with `SendMessage`"));
        // air-12k: the heartbeat is the failsafe, and roles.md promises no `stuck` condition.
        assert!(ROLES_MD.contains("the heartbeat is the failsafe"));
        assert!(!ROLES_MD.contains("the channel (stuck,"));
        // air-97z: roles.md states landing as a role boundary and as facts Air records, and
        // names no landing command. A repo with its own lander keeps it, so the prose that used
        // to prescribe `air land --all` here is asserted ABSENT, the same shape as the
        // awaiting_review check below. The deny-list line may still name `air land`: that is a
        // statement about what Air refuses a worker, not an instruction to a repo.
        assert!(ROLES_MD.contains("Landing is the coordinator's, not a worker's."));
        assert!(
            !ROLES_MD.contains("Landing is the coordinator's: `air land"),
            "roles.md must not prescribe a landing command (air-97z)"
        );
        // air-03w (owner, 2026-08-29): signalling on close is part of the worker role, and
        // the `landable` condition is the failsafe under it. The binary and the prose are one
        // file (`include_str!`), so this asserts on ROLES_MD and the equality above carries it
        // to disk. It names a FACT Air records, never a landing command, so it stands beside
        // air-97z's absence check above rather than against it.
        assert!(ROLES_MD.contains("Signal the coordinator when you close a bead"));
        assert!(ROLES_MD.contains("`landable` condition"));
        assert!(ROLES_MD.contains("bug `## Steps to Reproduce` + `## Acceptance Criteria`"));
        assert!(ROLES_MD.contains("epic `## Success"));
        // air-8zu: roles.md states what Air records and refuses, never one repo's closing
        // procedure. The adopter closes with proof and was being told to set awaiting_review by
        // a file it cannot edit. `awaiting_review` may appear only where the refusal lists
        // what the gate matches, never as an instruction.
        assert!(
            !ROLES_MD.contains("`bd update <id> -s awaiting_review`"),
            "roles.md must not prescribe a bead-status step"
        );
        assert!(ROLES_MD.contains("is the repo's own flow, in its CLAUDE.md"));
        // air-8gj: the worktree fence is Air's, not the harness's. Both halves are pinned,
        // because dropping either leaves roles.md promising a block that is not there — which
        // is the direction a rules file must not fail in. The claim that used to stand here
        // ("Editing the main checkout is blocked natively") is asserted ABSENT: it was true of
        // `claude --worktree` and is false without it.
        assert!(ROLES_MD.contains("resolved path leaves your worktree is denied by Air's"));
        // air-ahl: the tracked requirement and its route, both pinned. The route is the half
        // that must not go missing: a worker who meets the refusal has already committed, so
        // advice living only in the refusal arrives after the thing it prevents.
        //
        // Through `flat`, because roles.md is hard-wrapped and a pin that spans a wrap fails
        // on the wrap rather than on the rule. That happened twice this round (air-zth, then
        // this), so it is a helper now rather than a third carefully shortened substring.
        let roles = flat(ROLES_MD);
        assert!(roles.contains("It also has to be tracked by git"));
        assert!(roles.contains("commit the digest WITHOUT a `Bead:` trailer"));
        // air-g5o: the coordinator states where a bead came from as a DECLARED field, and
        // states that the count attached to it refuses nothing. Both halves are pinned: a
        // rules file that names a count without saying it is not a gate is how a measurement
        // becomes a rule nobody decided on.
        assert!(ROLES_MD.contains("initiative: <CODE>"));
        assert!(ROLES_MD.contains("It is a count and there is no refusal attached to it"));
        // air-zth: the coordinator's context is the channel, so the reading is delegated by
        // default. Pinned with its removal condition, because a roles line with no way out is
        // the throttle the do-less rule exists to prevent (air-s7c).
        assert!(ROLES_MD.contains("Your context is the channel the owner and every worker reach"));
        // One line's worth: roles.md is hard-wrapped, so an assertion spanning a wrap fails on
        // the wrap rather than on the rule.
        assert!(ROLES_MD.contains(
            "round shows zero owner or worker messages waiting more than five minutes on the \
             coordinator."
        ));
        assert!(
            !ROLES_MD.contains("Editing\nthe main checkout is blocked natively")
                && !ROLES_MD.contains("the main checkout is blocked natively"),
            "roles.md must not promise the harness's block once the flag is gone (air-8gj)"
        );
        // air-u3l7: the reserves-nothing line states a consequence; between 2026-09-05 and
        // 2026-09-07 it was read, agreed with and worked around five times, once by the Stop
        // hook itself. What it was missing is the alternative, so the alternative is what is
        // pinned — a consequence with no procedure beside it reads as "be careful".
        assert!(ROLES_MD.contains("So put the craft notes on the bead, not in the message"));
        assert!(ROLES_MD.contains("bd comment <id> --file <notes>"));
        // air-uef: one queue, and it is beads. The owner inbox is not offered anywhere.
        assert!(ROLES_MD.contains("Every capture is triaged into a bead or dropped with a reason"));
        assert!(ROLES_MD.contains("labelled `owner` with the coordinator's recommendation"));
        assert!(
            !ROLES_MD.contains("inbox --owner"),
            "the owner inbox is gone (air-uef)"
        );
        assert!(
            !ROLES_MD.contains("--for owner"),
            "the owner audience is gone (air-uef)"
        );
        assert!(!ROLES_MD.contains("owner decision waiting"));
        // Two facts from the adopter's round, riding on the same file (owner, 2026-09-05).
        assert!(ROLES_MD.contains("Naming a bead at a worker reserves nothing"));
        assert!(ROLES_MD.contains("reads the tree alone and not git history"));
        // air-9ij (owner, 2026-09-06): the coordinator has to know where main still costs
        // something and where it has stopped costing anything. All three halves are pinned,
        // because dropping the last one leaves the adopter's freeze-main workaround standing.
        // air-84u (owner, 2026-09-06): decomposition is a duty in the active voice, and the
        // old phrasing is asserted ABSENT — it read as a property of a good queue, which is
        // exactly how air-80x sat undecomposed for hours with nobody having failed at
        // anything. Both halves, the same shape air-97z uses.
        assert!(ROLES_MD.contains("Decomposing an\nepic is something you go and do"));
        assert!(ROLES_MD.contains("the `decomposition` skill"));
        assert!(ROLES_MD.contains("The reading may be delegated to a background agent"));
        assert!(
            !ROLES_MD.contains("(epics decomposed;"),
            "roles.md must state decomposition as a duty, not as a property of the queue"
        );
        // air-kexg: two workers derived "a journal branch can land" from correct premises
        // and were wrong, so roles.md says it. Both halves pinned: the permission and the
        // constraint, because a reader who keeps only the first has a bypass.
        assert!(ROLES_MD.contains("whose only commits are journal entries lands without one"));
        assert!(ROLES_MD.contains("mixes them\nwith anything else needs a trailer"));
        assert!(ROLES_MD.contains("What main moving costs, and what it no longer costs"));
        assert!(ROLES_MD.contains("The coordinator's own commits move main exactly as a landing"));
        assert!(ROLES_MD.contains("main moving no longer retracts a CLOSE"));
        // air-80x.6: the verification lane, under Worker, as facts Air records and refusals
        // Air makes. Two sentences pinned the way air-03w and air-97z pinned theirs; and the
        // section names no cadence and no worker, because those are the repo's flow.
        let lane = ROLES_MD
            .split("### Verification lane")
            .nth(1)
            .and_then(|s| s.split("\n## ").next());
        assert!(
            lane.is_some(),
            "roles.md has a Verification lane section under Worker"
        );
        let lane = lane.unwrap();
        assert!(lane.contains("A lane is a worker session like any other"));
        // air-4noi: the permission names the condition it assumes. An adopter's lane resets
        // hard to main on every cut, so a bead it held between batches would live in a tree
        // the next cut wipes. Through `flat` because the clause spans a hard wrap, which has
        // broken this pin twice; and pinned as the CONDITION rather than the whole sentence,
        // because deleting the permission would be wrong for a lane that merges main forward.
        assert!(flat(lane).contains("it may hold one **if its worktree survives the cut**"));
        assert!(flat(lane).contains("Air reads neither"));
        assert!(lane.contains(
            "The close gate accepts a green at a verified commit\nthat contains `main` and every commit carrying the bead's trailer"
        ));
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
