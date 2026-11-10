# Starting a fleet without answering folder trust

Research for air-ekj, 2026-11-05, read against this tree at `b53ce1f`. Why: on 2026-11-05 the
pod trial (air-fzl) stalled for hours because `air coordinator` refused to start in the fresh
trial copy, `hasTrustDialogAccepted` being absent for it in `~/.claude.json`, and nobody was
watching the pane to run `claude` there once. The question is whether the dialog can be
answered by policy instead of by the owner at each new directory. Research only; nothing in
Air changes with this document.

The short answer: Claude Code has no flag, setting or environment variable that pre-accepts
folder trust for an interactive session. The one documented route is the hand-set key
`projects["<path>"].hasTrustDialogAccepted` in `~/.claude.json`, which the docs tell an agent
not to set on the user's behalf. Trust is keyed by path, so the cheap answer is already in the
trial script: `TRIAL_DIR` names the parent of the copy, and a fixed parent the owner trusted
once is trusted for every later copy at that path. Nothing to build.

## 1. Does it already exist?

### 1.1 In Claude Code: no

Every Claude Code page below was downloaded on 2026-11-05 and searched for "trust"; the
settings reference (449 KB) was searched whole, so "no key" is a count over the page, not a
reading of part of it.

- The dialog, and what it keys on. "Claude Code shows the trust dialog in interactive sessions
  only." In a repository the trust "covers the whole repository apart from any git repository
  nested inside it, such as a submodule. In a worktree, it uses the main checkout's root."
  Outside a repository it "covers any subdirectory of that directory apart from a git
  repository nested inside it, such as a clone." Started in the home directory, trust is "for
  the current session only" and never written.
  (https://code.claude.com/docs/en/permissions.md, "Project allow rules and workspace trust")
- The hand edit. "For the rows that need this exact folder trusted, trust it by hand: set
  `projects["<path>"].hasTrustDialogAccepted` to `true` in `~/.claude.json`, where `<path>` is
  the repository root, or the folder itself outside a repository." (same page, "What runs
  before you trust a folder"; the same instruction is in the MCP page under "Trust without a
  dialog" and in three error entries.)
- No setting. The settings reference has no key that records, skips or pre-accepts folder
  trust. The nearest, `skipDangerousModePermissionPrompt`, skips the confirmation before
  `bypassPermissions` mode and says "An untrusted repository can't skip the dialog for you."
  (https://code.claude.com/docs/en/settings-reference.md, that key)
- No flag for `claude` itself. The CLI reference's only trust mention is `--bg`, which "checks
  workspace trust for the directory before it starts" and, where no dialog can appear, "exits
  with a `Workspace not trusted` error instead". (https://code.claude.com/docs/en/cli-reference.md;
  permissions.md, same section)
- One subcommand has a flag. `claude ultrareview --trust-workspace` trusts "the current folder
  for this run only, without asking, so a script or CI job can review a checkout you haven't
  trusted" (v2.1.309), and `CLAUDE_CODE_DISABLE_ULTRAREVIEW_TRUST_CHECK=1` does the same for
  that command. Neither applies to a session. (https://code.claude.com/docs/en/ultrareview.md,
  flags table; https://code.claude.com/docs/en/env-vars.md)
- Headless sessions never ask. "A `claude -p` run or an SDK session never shows it", and for
  hooks "treats the folder as trusted". (permissions.md, same section;
  https://code.claude.com/docs/en/hooks.md, "Workspace trust")
- What the docs tell an agent. The error entry for an unrestored teammate definition ends: "the
  user needs to run Claude Code in that folder once and accept the trust dialog (the --debug log
  names the folder); do not change trust settings on the user's behalf."
  (https://code.claude.com/docs/en/errors.md, "Teammate agent definition not restored")

### 1.2 In the field

The field has asked for a flag and been told to set the key. Checked at the source on
2026-11-05 (the issue state and the maintainer's words, through the GitHub API):

- anthropics/claude-code #12980, "Add option to skip trust prompt when
  .claude/settings.local.json allows it", closed completed. The maintainer: "Trust is now shared
  across git worktrees of the same repository (since 2.1.63) ... For other pre-trusting needs
  (e.g. non-interactive setups), you can also set `projects["<repo-root>"].hasTrustDialogAccepted:
  true` in `~/.claude.json`."
- #23109, "Trusted workspace patterns to skip trust prompt for git worktrees", closed completed
  without the requested glob setting: "workspace trust is now keyed on the repository's main
  checkout ... Trusting a parent directory also covers plain (non-repo) subdirectories under it."
- #28506, "--dangerously-skip-permissions does not bypass workspace trust prompt", closed; the
  bypass flag does not cover trust.
- Claude Code changelog 2.1.232: "Fixed nested git repositories inheriting trust from a parent
  directory; each repository now requires its own trust confirmation." So option B below once
  worked and was closed as a defect.

Orchestrators that start sessions in fresh worktrees or clones have all landed on the same
answer, writing the key for the launch directory's git root, honouring `CLAUDE_CONFIG_DIR`, and
merging into the file rather than overwriting it. As reported by the web search (URLs read by
the search, not re-opened here): kunchenguid/firstmate #3379 (`bin/fm-claude-trust.sh`),
Untrivial-ai/agent-orchestrator #4888, asheshgoplani/agent-deck #2673, blooop/devlaunch #617
(a devcontainer post-create step), and a dev-container post at
paulsprogrammingnotes.com (2026-06). Two drive the dialog with tmux keystrokes instead
(taxiway-sh/taxiway #82 on Gas Town 1.1.0; PaulSnow/agentd-kit #32), and both note that the
dialog's default key, Enter, picks "No, exit". Several user-filed issues report the key
reverting to false or being read stale (#36403, #43958, #65864, #71562, #97888, #90220), which
is the cost of a file two writers share. Nothing ties Overstory, Vibe Kanban or claude-squad to
the key.

### 1.3 In Air

- The launcher reads the key before any session starts and refuses with the fix when it is not
  `true`; it never writes it (`crates/cli/src/cmd/launch.rs` `trust_refusal`, air-z41). Before
  that, five sessions started at once each stopped on the dialog.
- `scripts/trial.sh` takes `TRIAL_DIR` as the copy's parent: `parent=${TRIAL_DIR:-$(mktemp -d
  "${TMPDIR:-/tmp}/air-trial.XXXXXX")}`, copy at `$parent/minimal`, refused when it exists
  (`scripts/trial.sh:34-36`). Without it every trial gets a new random path, and so a new
  dialog.
- This machine's `~/.claude.json` has three entries that matter, each accepted by hand: the main
  checkout and two trial copies at random `$TMPDIR/air-trial.*/minimal` paths, one of them the
  pod trial's. No worktree has an entry, as the docs say.

## 2. What the dialog gates in a fleet session

Air's sessions are interactive, so the interactive column applies. Until the folder is
trusted: hooks from every settings file are held back, including the user's own
(hooks.md, "Workspace trust"), so `air hook` would not run; `permissions.allow` in the project
file is not used; servers in `.mcp.json` ask before connecting. Air already puts its allow rules
and `enabledMcpjsonServers` on `--settings`, which an untrusted folder honours
(settings-reference.md, `enabledMcpjsonServers`, scope), so what the dialog protects in a fleet
is the hooks and the project file's rules. The dialog "lists the rules and directories the folder
would grant so you can review them first" (permissions.md); it is the owner's review of what a
folder may run, which is why Air's launcher refuses rather than answers.

## 3. The options

| Option | Cost | What it weakens | Owner in the loop |
|---|---|---|---|
| A. A documented flag or setting that pre-trusts a directory | none | nothing | yes | 
| B. Trust a parent directory | none | nothing | yes |
| C. A trusted base for scratch copies (`TRIAL_DIR` fixed) | one accept per machine; the old copy removed by hand | the path stays trusted whatever is copied there | yes, once |
| D. Air writes the key | a few lines in the launcher | the one decision Claude Code reserves for the person; a race with Claude Code's own writes to the file | no |
| E. Headless sessions (`-p`, SDK) | a different launcher | hooks run untrusted; no terminal to watch or type into | no |

- A does not exist for a session (§1.1). The precedent to ask for is `ultrareview`'s
  `--trust-workspace`, scoped to one run; a session-level flag of the same shape would end the
  launcher's refusal, which names that as its removal condition.
- B fails on the docs: parent trust stops at "a git repository nested inside it, such as a
  clone", and the trial copy is `git init`ed in its parent (`scripts/trial.sh:42`). A trusted
  `$TMPDIR` would not count, and "in an interactive session Claude Code shows the trust dialog
  for it" anyway.
- C works with no change to Air. Trust is keyed by the path string; a copy made again at the
  same path is a trusted folder. The owner accepts once per machine, in a directory they chose
  and whose content is Air's own example, so the review the dialog exists for has been done.
  What it weakens: the path stays trusted after the copy is deleted and remade, which is the
  same standing trust every repository on the machine has. The cost is in the script: its
  clean-up loop removes only `$TMPDIR/air-trial.*` (`scripts/trial.sh:19-32`), so a fixed
  `TRIAL_DIR` needs `rm -rf "$TRIAL_DIR/minimal"` and the old sessions killed by hand, or the
  loop extended to `$TRIAL_DIR`. One loop if the owner wants it.
- D is the one the docs rule out. The file is one Claude Code "writes for itself; you don't need
  to edit it" (https://code.claude.com/docs/en/settings.md), the errors page tells an agent not
  to change trust on the user's behalf, and a key written by `air fleet up` would trust whatever
  directory the coordinator was pointed at, with nobody reviewing what its settings activate.
  It also takes the owner out of the loop, against CLAUDE.md's standing rule.
- E is ruled out by the same rule: every Air session is an interactive terminal the owner can
  watch and type into. It is also the mode in which repository hooks run in a folder nobody
  trusted, which is the opposite of what the dialog is for.

## 4. Recommendation and verdict

Use C and build nothing. For a trial on this or any machine, set `TRIAL_DIR` to a fixed
directory, run `claude` once in `$TRIAL_DIR/minimal` after the first `make trial` and accept,
and every later trial at that path starts without the dialog. The pod trial is the same case:
the copy and every session are on the laptop, and the pods only run commands. The do-less
answers: the recorded failure is one stall (air-fzl, 2026-11-05) and air-z41's five prompts
before it; the fix is a documented environment variable the script already has, so there is no
mechanism to add and no removal condition to write. The launcher's refusal keeps its own
removal condition, "when Claude Code offers a supported way to pre-accept trust for a directory
the owner already trusted", and §1.1 shows that day has not come for sessions.

Two small follow-ups, both documentation, filed as the coordinator sees fit: a sentence in
`docs/design.md` §9.1 and in `scripts/trial.sh`'s header saying that a fixed `TRIAL_DIR`
trusted once answers the dialog for every later trial; and, if the owner wants `make trial` to
clear a fixed `TRIAL_DIR` as it clears `$TMPDIR`, one more loop in the script.

Superseded in part on 2026-11-07 (air-zau). The dialog stopped the trial twice more after this
was written, and the owner ruled that the trial should need no step at all. Option D was built
narrowed to what this section objected to least: only `scripts/trial.sh` writes the key, for the
one path it has just built from `examples/minimal`, before any session starts, through a temp
file and a rename so Claude Code's own writes are never raced
(`scripts/trial/trust-copy.py`). The launcher's refusal and its removal condition are
unchanged, and a repository clone still takes one answer per machine. Removed when Claude Code
offers a session-level flag that pre-trusts a directory.
