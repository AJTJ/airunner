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

/// The documents Air ships beside `roles.md`, written to `.air/docs/` by `air install --write`
/// (air-g9dx, owner 2026-09-29): how Air works, what has gone wrong before and why, and how to
/// adopt it. An adopter's coordinator wrote 557 lines of findings partly guessing at mechanisms
/// it could not read, and the defect skill reads two of these. Embedded like `roles.md`, so the
/// copy a repo holds is the one its binary was built with; nothing injects them into a
/// session, they are read on demand. Removed when an adopter can read Air's repository itself.
pub const DOCS: &[(&str, &str)] = &[
    ("design.md", include_str!("../../../../docs/design.md")),
    (
        "solved-issues.md",
        include_str!("../../../../docs/solved-issues.md"),
    ),
    (
        "adopting-air.md",
        include_str!("../../../../docs/rules/adopting-air.md"),
    ),
];

/// The shipped documents that are missing from `air_dir`'s `docs/` or differ from this
/// binary's copies, for `air doctor` (air-g9dx). Empty when `roles.md` is absent too, since
/// then Air was never installed here.
pub fn docs_drift(air_dir: &Path) -> Vec<String> {
    if !air_dir.join("roles.md").exists() {
        return Vec::new();
    }
    DOCS.iter()
        .filter(|(name, text)| {
            std::fs::read_to_string(air_dir.join("docs").join(name))
                .ok()
                .as_deref()
                != Some(*text)
        })
        .map(|(name, _)| format!(".air/docs/{name}"))
        .collect()
}

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
    // air-g9dx: shipped once `.air/docs/` carries the two documents it reads.
    (
        "air-defect",
        include_str!("../../../../.claude/skills/defect/SKILL.md"),
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

/// Every surface change since Air started recording them, one file each under
/// `crates/cli/surface.d/NNNN-<id>.rs`, folded by `build.rs` in filename order (air-38q: two
/// beads appending to one array conflicted at the same line on every landing). A notice is a
/// new file there, never an edit to this array; never edit an id. A lane appends the notice
/// only; the release row that covers it is appended by the coordinator at round end (air-mir),
/// and `make release` refuses until the two agree.
pub const SURFACE: &[SurfaceChange] = include!(concat!(env!("OUT_DIR"), "/surface.rs"));

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
    // 2026-09-26: the coordinator's own commits land with no bead, bd 1.3.0 pin, air reclaim,
    // air bd-server and server mode for new projects. 0.4.4's trial found the first.
    ("0.4.5", 35, 128),
    // 2026-09-26: workers close with `air close`, which runs the hand-over gate itself. 0.4.5's
    // trial found a close the text-matching gate missed (a `bd close` on its own line).
    ("0.4.6", 36, 129),
    // 2026-09-26: 0.4.6's trial fixes; the text-matching close gate deleted (workers close with
    // `air close` and are denied bd writes); bd's own server start off; port ownership checked.
    ("0.4.7", 37, 141),
    // 2026-09-26: air close judges the bead by its `Bead:` trailer commits, so a worker can start
    // its next bead while the lane verifies the last. 0.4.7's trial found the head-based refusal.
    ("0.4.8", 38, 142),
    // 2026-09-26: every batch starts from main; the causes behind past trial workarounds
    // (precheck hold told, no merge-main advice under a lane, landing green, wording).
    ("0.4.9", 39, 148),
    // 2026-09-26: 0.4.9's trial: a dropped branch stays out until it moves, the drop notice
    // names the peer, ready lists rotate per worker, bare filenames settle acceptance clauses.
    ("0.4.10", 40, 154),
    // 2026-09-29: the round on an adopter's 0.4.10 findings (42 landings); its live trial
    // passed 8/8.
    ("0.4.11", 41, 196),
    // 2026-10-21: notices from the commands, red batches hold every member, .air/docs
    // shipped, batch cut --without/--only; first release with binaries.
    ("0.4.12", 42, 202),
    // 2026-10-22: Dolt pinned at 2.3.5 in `air doctor`; bd and Dolt install from their releases.
    ("0.4.13", 43, 203),
    ("0.4.14", 44, 220),
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

/// Pure: is the newest parity sweep newer than every change to code and roles (air-g9dx)?
///
/// `sweep` is the newest closed bead titled "Parity sweep" with the commit on main that names
/// it; `later` is every non-merge commit after that one which changes `crates/` or `docs/rules/`
/// and names a different bead, as (sha, subject). The docs ship in the binary now, so a release
/// cut after a code change nobody swept ships prose about behaviour that is gone: the round of
/// 2026-09-28 needed two sweeps (23 stale statements, then more). Removed after two releases in
/// a row whose sweep found nothing stale.
pub fn parity_verdict(
    sweep: Option<(&str, &str)>,
    later: &[(String, String)],
) -> Result<(), String> {
    const FIX: &str = "file a bead titled \"Parity sweep ...\" that brings the docs, skills, \
                       roles and README to what the code does, and land it before the release";
    let Some((bead, sha)) = sweep else {
        return Err(format!(
            "no closed parity sweep has a commit on main: {FIX}."
        ));
    };
    match later.first() {
        None => Ok(()),
        Some((c, subject)) => Err(format!(
            "the newest parity sweep, {bead} at {}, predates {} change(s) to crates/ or \
             docs/rules/, the newest {} \"{subject}\": {FIX}.",
            sha.get(..8).unwrap_or(sha),
            later.len(),
            c.get(..8).unwrap_or(c),
        )),
    }
}

/// The newest sweep as (bead, sha on main), and the later commits as (sha, subject).
type ParityFacts = (Option<(String, String)>, Vec<(String, String)>);

/// The facts [`parity_verdict`] reads, from bd and git.
fn parity_facts(repo: &Path) -> Result<ParityFacts, String> {
    use air_bd::WorkLedger;
    let bd = super::claim::bd_for(repo);
    let closed = bd
        .by_status("closed")
        .map_err(|e| format!("bd list closed: {e}"))?;
    let behind = |sha: &str| -> usize {
        crate::git::run(repo, &["rev-list", "--count", &format!("{sha}..main")])
            .ok()
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(usize::MAX)
    };
    let sweep = closed
        .iter()
        .filter(|i| i.title.to_lowercase().starts_with("parity sweep"))
        .filter_map(|i| super::batch::in_main(repo, &i.id).map(|sha| (i.id.clone(), sha)))
        .min_by_key(|(_, sha)| behind(sha));
    let Some((bead, sha)) = &sweep else {
        return Ok((None, Vec::new()));
    };
    let text = crate::git::run(
        repo,
        &[
            "log",
            "--no-merges",
            "--format=%H%x1f%s%x1f%B%x1e",
            &format!("{sha}..main"),
            "--",
            "crates",
            "docs/rules",
        ],
    )
    .map_err(|e| format!("git log: {e}"))?;
    let later = text
        .split('\u{1e}')
        .filter_map(|rec| {
            let mut it = rec.trim_start_matches('\n').splitn(3, '\u{1f}');
            let c = it.next()?.trim().to_string();
            let subject = it.next()?.to_string();
            let body = it.next().unwrap_or("");
            let names_sweep = super::attribution::trailer_ids(body)
                .iter()
                .any(|b| b == bead);
            (!c.is_empty() && !names_sweep).then_some((c, subject))
        })
        .collect();
    Ok((sweep, later))
}

/// `air release-check`: exit 2 with the row to append when the surface has outrun the
/// releases, or when the docs have not been swept since the last code change. Run by
/// `make release`, never by `make verify`.
pub fn release_check_cmd(repo: &Path) -> i32 {
    let parity = parity_facts(repo).and_then(|(sweep, later)| {
        parity_verdict(
            sweep.as_ref().map(|(b, s)| (b.as_str(), s.as_str())),
            &later,
        )
    });
    if let Err(e) = parity {
        eprintln!("release-check: {e}");
        return 2;
    }
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

/// The `"project"` `air install --write` adds to an existing `.claude/air.json` that has none:
/// the main checkout's directory name, which is what `air init` writes (owner, 2026-09-26).
/// Without it session names fall back to the bead prefix. `None`
/// when the key is there, whatever its value, or when the file is not a JSON object.
pub fn project_to_add(air_json: &Value, main_dir_name: &str) -> Option<String> {
    let m = air_json.as_object()?;
    (!m.contains_key("project") && !main_dir_name.is_empty()).then(|| main_dir_name.to_string())
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
    /// `"project"` added to `.claude/air.json`, which had none (owner, 2026-09-26).
    project_added: Option<String>,
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
because `air install` merges and never removes another tool's hook unless asked. Remove it with \
`air install --write --remove-stale`.";

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

/// `settings` without the hook entries [`stale_hooks`] names, and without any group or event
/// left empty by that (air-e07q). Pure. `air install --write --remove-stale` applies it, so
/// removing a hook that contradicts Air is Air's command rather than a hand edit of
/// `.claude/settings.json`; without the flag nothing another tool wrote is removed.
pub fn drop_stale(mut settings: Value) -> Value {
    let stale: Vec<String> = stale_hooks(&settings)
        .into_iter()
        .map(|h| h.command)
        .collect();
    if stale.is_empty() {
        return settings;
    }
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return settings;
    };
    for groups in hooks.values_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        for group in groups.iter_mut() {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                list.retain(|h| {
                    h.get("command")
                        .and_then(Value::as_str)
                        .is_none_or(|c| !stale.iter().any(|s| s == c))
                });
            }
        }
        groups.retain(|g| {
            g.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|l| !l.is_empty())
        });
    }
    hooks.retain(|_, groups| groups.as_array().is_none_or(|g| !g.is_empty()));
    settings
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

pub fn run(repo: &Path, write: bool, json: bool, pin_arg: PinArg, remove_stale: bool) -> i32 {
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
    let merged = merge_hooks_with(before_settings.clone(), &hook_command(pin.as_deref()));
    // What the merge leaves in place and should not (air-b5k). With `--remove-stale` it goes
    // (air-e07q), and each removal is said once; otherwise it is reported as before.
    let removed = if remove_stale {
        stale_hooks(&merged)
    } else {
        Vec::new()
    };
    let after_settings = if remove_stale {
        drop_stale(merged)
    } else {
        merged
    };
    for h in &removed {
        eprintln!(
            "air install: {} the stale {} hook `{}`",
            if write { "removed" } else { "would remove" },
            h.event,
            h.command
        );
    }
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
    let air_json_path = repo.join(".claude/air.json");
    let air_json = if air_json_path.is_file() {
        read_json(&air_json_path).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let main_dir_name = super::worktree::main_checkout(&repo)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let project_added = project_to_add(&air_json, &main_dir_name);

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
        project_added: project_added.clone(),
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
            if let (Some(p), Value::Object(mut m)) = (&project_added, air_json.clone()) {
                m.insert("project".into(), Value::String(p.clone()));
                write_json(&air_json_path, &Value::Object(m))?;
            }
            std::fs::create_dir_all(&air_dir).map_err(|e| format!("{}: {e}", air_dir.display()))?;
            std::fs::write(air_dir.join("roles.md"), ROLES_MD)
                .map_err(|e| format!("{}: {e}", air_dir.display()))?;
            let docs = air_dir.join("docs");
            std::fs::create_dir_all(&docs).map_err(|e| format!("{}: {e}", docs.display()))?;
            for (name, text) in DOCS {
                std::fs::write(docs.join(name), text)
                    .map_err(|e| format!("{}: {e}", docs.display()))?;
            }
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
        if let Some(p) = &plan.project_added {
            s.push_str(&format!(
                "air.json: {} \"project\": \"{p}\" (it had none; session names use it)\n",
                if plan.written { "added" } else { "will add" }
            ));
        }
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

    /// Owner, 2026-09-26: install adds `"project"` when it is missing and never changes one
    /// that is there.
    #[test]
    fn install_adds_a_missing_project_and_leaves_one_that_is_there() {
        assert_eq!(
            project_to_add(&json!({"worker_deny": []}), "minimal"),
            Some("minimal".to_string())
        );
        assert_eq!(project_to_add(&json!({"project": "mine"}), "minimal"), None);
        assert_eq!(project_to_add(&json!({"project": ""}), "minimal"), None);
        assert_eq!(project_to_add(&Value::Null, "minimal"), None);
    }

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
        // air-wvx: the coordinator offers named workflows and takes the owner's pick; a worker
        // can always make a file change, so "only when no worker can" named no real case.
        assert!(has("offer a menu of named workflows"));
        assert!(has("Write the pick in your journal"));
        assert!(!has("only when no worker can make the change"));
        // air-d73: the coordinator may answer a launched session's start-up prompt through
        // tmux, after asking the owner once; folder trust stays the owner's.
        assert!(has("Shall I accept these on your behalf?"));
        assert!(has("never the folder-trust dialog"));
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
        // air-g9dx: the shipped documents are named where every role reads.
        assert!(has(
            "How Air works is in `.air/docs/design.md`, and what has gone wrong before and why is in `.air/docs/solved-issues.md`"
        ));
        // air-gcjp: no close without a commit naming the bead.
        assert!(has("Every bead closes on a commit that names it"));
        // air-jcfb: no role waits on the question prompt, and every launcher denies it.
        assert!(has(
            "No role uses the question prompt (`AskUserQuestion`), and every launcher denies it."
        ));
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
        // Owner, 2026-09-26: a worker closes with `air close`, which runs the gate itself; the
        // text matcher on a raw `bd close` missed a multi-line call in the 0.4.5 live trial.
        assert!(has("air close <id> --reason-file <proof>"));
        assert!(!has("bd close"));
        assert!(has("You need not merge main to stay batch-ready"));
        // air-6el: no command line in the shipped text hardcodes `main` as the branch. Where a
        // repo sets `base_branch`, a copied `git merge main` fails (two workers in the
        // 2026-11-06 trial); the sequence says `<base branch>` and Every role says where to
        // read the name.
        assert!(
            !has("git merge main"),
            "a command line in roles.md hardcodes main"
        );
        assert!(has("git merge <base branch>"));
        assert!(has("`air status` prints `base branch: <name>`"));
        assert!(has("commit forward and never amend"));
        // air-w18h: no fence; the roles say where to work. The harness claim stays absent
        // because it would promise a block that is not there (air-8gj).
        assert!(has(
            "Work only in your worktree, never in the main checkout, and use your scratchpad"
        ));
        assert!(!has("denied by Air's PreToolUse hook"));
        // air-b1h8: the lane's allow rules match only the bare line.
        assert!(has(
            "Run these and `air land` bare, with no pipe or redirect"
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
        assert!(has("When an epic has no children, decompose it"));
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
        // The cut resets the lane's branch (0.4.8 trial), so no bead survives one (air-4noi).
        assert!(flat_lane.contains("work of the lane's own does not survive a cut"));
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
    /// air-38q: the array IS the directory. Every `*.rs` fragment is one notice, in filename
    /// order, and its name after the ordinal is the notice's id, so a fragment that was not
    /// folded, or folded out of order, shows here and not at an adopter's `air install`.
    #[test]
    fn surface_is_the_fragments_in_filename_order() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("surface.d");
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".rs"))
            .collect();
        names.sort();
        assert_eq!(names.len(), SURFACE.len(), "{names:?}");
        for (name, change) in names.iter().zip(SURFACE) {
            let (ordinal, rest) = name.split_once('-').unwrap();
            assert!(
                ordinal.len() == 4 && ordinal.bytes().all(|b| b.is_ascii_digit()),
                "{name}: the ordinal is four digits"
            );
            assert_eq!(rest.trim_end_matches(".rs"), change.id, "{name}");
        }
        let mut ids: Vec<&str> = SURFACE.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SURFACE.len(), "an id repeats");
    }

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

    /// air-e07q: `--remove-stale` takes out exactly the stale entries, drops what that
    /// empties, and leaves Air's hooks and every other hook alone.
    #[test]
    fn drop_stale_removes_only_the_stale_hooks() {
        let with = json!({"hooks": {
            "SessionStart": [{"hooks": [
                {"type": "command", "command": "bd prime --hook-json"}]}],
            "Stop": [{"hooks": [
                {"type": "command", "command": "bd ready --json"},
                {"type": "command", "command": "/usr/local/bin/bd prime"}]}]
        }, "permissions": {"allow": ["Bash(ls *)"]}});
        let merged = merge_hooks(with);
        let out = drop_stale(merged.clone());
        assert!(stale_hooks(&out).is_empty(), "{out}");
        // The other tool's non-stale hook and Air's own survive; permissions are untouched.
        let text = out.to_string();
        assert!(text.contains("bd ready --json"), "{out}");
        assert!(text.contains("air hook"), "{out}");
        assert_eq!(out["permissions"], merged["permissions"]);
        // Nothing stale: unchanged.
        assert_eq!(drop_stale(out.clone()), out);
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

    /// air-g9dx: the release refuses a parity sweep older than the last change to code or
    /// roles, and one that is missing, naming the fix; a sweep with nothing after it passes.
    #[test]
    fn release_refuses_a_parity_sweep_older_than_the_code() {
        let later = vec![(
            "25499223aa".to_string(),
            "red batch: members stay out".to_string(),
        )];
        let e = parity_verdict(Some(("zz-sweep", "ec1965fbcc")), &later).unwrap_err();
        assert!(e.contains("zz-sweep at ec1965fb"), "{e}");
        assert!(e.contains("25499223"), "{e}");
        assert!(e.contains("Parity sweep"), "{e}");
        assert!(
            parity_verdict(None, &[])
                .unwrap_err()
                .contains("Parity sweep")
        );
        assert!(parity_verdict(Some(("zz-sweep", "ec1965fbcc")), &[]).is_ok());
    }

    /// air-g9dx: every function the design doc's lifecycle table cites as `module::function`
    /// exists, so a rename fails `make verify` instead of shipping a table that names nothing.
    #[test]
    fn every_function_the_lifecycle_table_cites_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let design = std::fs::read_to_string(root.join("docs/design.md")).unwrap();
        let start = design.find("### The bead and branch lifecycle").unwrap();
        let end = design[start..].find("### 6.1").map(|e| start + e).unwrap();
        let table = &design[start..end];
        let mut cited = 0;
        for tok in table.split('`').skip(1).step_by(2) {
            let Some((module, func)) = tok.split_once("::") else {
                continue;
            };
            if !func.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            let files: Vec<std::path::PathBuf> = if module == "air_hooks" {
                std::fs::read_dir(root.join("crates/hooks/src"))
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .collect()
            } else {
                vec![root.join("crates/cli/src/cmd").join(format!("{module}.rs"))]
            };
            let found = files.iter().any(|f| {
                std::fs::read_to_string(f).is_ok_and(|t| t.contains(&format!("fn {func}(")))
            });
            assert!(
                found,
                "the lifecycle table cites `{tok}`, which is not a function"
            );
            cited += 1;
        }
        assert!(cited >= 10, "only {cited} citations read; the table moved?");
    }

    /// air-g9dx: a name `mechanisms::DELETED_NAMES` holds appears in the shipped documents and
    /// skills only in a paragraph or list item that says it was deleted or removed. The
    /// register (`solved-issues.md`) is left out: every entry there is history by design.
    #[test]
    fn deleted_names_appear_only_where_the_deletion_is_said() {
        let texts: Vec<(&str, &str)> = DOCS
            .iter()
            .filter(|(n, _)| *n != "solved-issues.md")
            .copied()
            .chain(SKILLS.iter().copied())
            .collect();
        for (file, text) in texts {
            let mut blocks: Vec<String> = Vec::new();
            for line in text.lines() {
                let t = line.trim_start();
                if t.is_empty() || t.starts_with("- ") || t.starts_with("| ") {
                    blocks.push(String::new());
                }
                if let Some(b) = blocks.last_mut() {
                    b.push_str(line);
                    b.push('\n');
                } else {
                    blocks.push(format!("{line}\n"));
                }
            }
            for block in &blocks {
                let lower = block.to_lowercase();
                for name in crate::cmd::mechanisms::DELETED_NAMES {
                    if lower.contains(&name.to_lowercase()) {
                        assert!(
                            ["delet", "remov", "no longer", "gone"]
                                .iter()
                                .any(|w| lower.contains(w)),
                            "{file}: `{name}` outside a line saying it was deleted:\n{block}"
                        );
                    }
                }
            }
        }
    }

    /// air-g9dx: this repository commits what `air install --write` writes for each shipped
    /// skill, and a launcher refuses while that output is uncommitted, so the committed copy
    /// must be the installer's output byte for byte.
    #[test]
    fn committed_skill_copies_are_what_install_writes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (name, text) in SKILLS {
            let on_disk =
                std::fs::read_to_string(root.join(".claude/skills").join(name).join("SKILL.md"))
                    .unwrap();
            assert_eq!(on_disk, skill_with_name(text, name), "{name}");
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
        // air-g9dx: and `air-defect`, once the documents it reads ship in `.air/docs/`.
        assert_eq!(SKILLS.len(), 3);
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
