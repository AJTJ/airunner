# Agent roles and confinement: coordinator, worker, and what Claude Code can enforce per worktree

**Researched 2026-08-20.** Official docs at `https://code.claude.com/docs/en/*.md` were fetched
the same day and every doc claim below names the page and section. the adopter is cited at commit
`71191e0`, Metis at `6745810`; both were read only. Method: the `explore` skill (protocol, external
sources first, then the project's own docs, then a dialectic).

Commissioned in `docs/decisions.md` (2026-08-20, research (b)): document each role, and find out
whether workers should stay CLAUDE.md-driven sessions or be "invoked in a very specific space with
very specific commands", and what Claude Code can enforce.

## 0. Protocol (pre-registration)

Question. (1) What is each of Air's two roles allowed, required, and forbidden to do, and where
does each duty come from? (2) How far can Claude Code mechanically confine a session to its role,
per worktree?

Sub-questions: (a) which settings files does a worktree session actually read; (b) which denials
survive `--dangerously-skip-permissions`; (c) can a hook deny reliably, and can it know the role;
(d) what does `--agent` give a main-thread session; (e) what does adopter enforce today and what
went wrong; (f) how does Metis scope a session.

Priors stated before reading: project and local settings are per-checkout, so a worktree can
carry its own deny list; hooks are the only role-aware layer; `--dangerously-skip-permissions`
defeats deny rules. Falsifiers: a doc saying a worktree shares the main checkout's local
settings; a doc saying deny rules hold in bypass mode.

Two of three priors were wrong (§3 rows 2 and 1). That is why the recommended design in §5 leans
on launch flags and the hook rather than on per-worktree settings files.

## 1. The one-paragraph finding

Role is a property of the checkout. The coordinator is the session whose cwd is the main
checkout; a worker is a session whose cwd is a linked worktree. the adopter has exactly one
mechanical role test, `[ -f .git ]`, and exactly one hard role gate, `land.sh` refusing to run
outside the main checkout or under `CLAUDECODE` (§2.3). Everything else about roles is prose, and
the recorded failures are all the coordinator drifting into worker work, never the reverse
(§2.4). Claude Code today enforces the worker-side boundary natively: a session started with
`--worktree` cannot edit, run commands in, or redirect git into the main checkout, and that check
cannot be turned off (§3 row 8). What Claude Code cannot do is give a worktree its own settings
file: since v2.1.211 `settings.local.json` resolves to the main checkout for every worktree, and
project `settings.json` is whatever the branch carries. Per-role confinement therefore has to be
applied at launch (`--disallowedTools`, `--append-system-prompt-file`, `--settings`, or a
`.claude/agents/<role>.md` run with `--agent`) or decided inside `air hook`, which already sees
`cwd` and derives `main` vs worktree name. The layered design in §5 uses the native worktree
isolation for the worker fence, a launcher-supplied deny list for the handful of coordinator
commands, and `air hook` for the role-aware, bd-shaped checks, advisory first.

## 2. The roles as adopter runs them

### 2.1 How a session knows its role

| Signal | Coordinator (main checkout) | Worker (worktree) | Source |
|---|---|---|---|
| `.git` | directory | file | `adopter/docs/rules/worktree-protocol.md:11-16`; same test in `scripts/land.sh:485-488`, `scripts/pre-commit:35-39`, `Makefile:175,213` |
| `git rev-parse --git-dir` vs `--git-common-dir` | equal | differ | `worktree-protocol.md:17-20`; Air: `crates/ledger/src/paths.rs:9-33` |
| `BEADS_ACTOR` | falls back to git identity unless set (`BEADS_ACTOR=coordinator` by hand) | worktree name, written by `make worktree-env` into `.claude/settings.local.json` `env` | `worktree-protocol.md:25-33`; `main-agent-protocol.md:75-115`; `Makefile:209-214` |
| `CLAUDECODE` | set | set | Not a role test: it separates agent from owner (`scripts/pre-commit:36-38`) |
| Air's `worker` name | `main` | directory name | `crates/ledger/src/paths.rs:37-62`, computed from the hook's `cwd` (`crates/cli/src/cmd/hook.rs:93-102`) |

"Coordinator" is not an identity in any adopter code path. It is "a session that stays
available, holds no lane, and answers" (`docs/rules/fleet-runs.md:22-25`), and the protocol tells
every session to say which checkout it is in as its first message (`worktree-protocol.md:22-23`).

### 2.2 Duties

Sources: F = adopter file at `71191e0`; A = Air doc. Decisions dated 2026-08-18/20 are in
`docs/decisions.md`.

| Duty | Coordinator | Worker | Source |
|---|---|---|---|
| Commit | **Forbidden** in main ("never `git add`/`git commit`"; `git add -A` with agents live is worse) | **Required**, small and often, on own branch | F `main-agent-protocol.md:19-28`; F `worktree-protocol.md:41-49`; A `rules/worktree-protocol.md §2` |
| Push | Forbidden (owner only) | Forbidden | F `main-agent-protocol.md:31-32`; F `cmd-guard.py:423-424`; A `rules/worktree-protocol.md §2` |
| Merge `main` into branch | n/a | **Required**, early, resolve conflicts yourself | F `worktree-protocol.md:217-232`; A `rules/worktree-protocol.md §6` |
| Land (`make land` / `air land`) | Runs it, or the owner does; "never merge to main on your own initiative" | **Forbidden**: "You do not run it, and you never push" | F `main-agent-protocol.md:31-32`; F `worktree-protocol.md:231-232`; A `plans/0001-first-slice.md §4` |
| Triage captures into beads, write acceptance, choose lanes | **Required** (inline, for now) | **Forbidden**: "workers capture, they do not file" | A `decisions.md` 2026-08-18 answer 3; A `plans/0002 §5` |
| `bd create` | Allowed (after triage, `--validate` on) | Forbidden by Air's decision; **not gated** in adopter (`make note` is free from any worktree, `cmd-guard.py:82-84` guards only `--notes/--description` replacement) | A `decisions.md` 2026-08-18; F `Makefile:424-437`; F `cmd-guard.py:82-84,504-529` |
| Capture a need (`make note` / `air capture`) | Allowed | **Required** whenever something is discovered outside the bead | A `decisions.md` 2026-08-18 answer 3; F `Makefile:424-431` |
| Claim beads | Only with a distinguishing assignee, and not a lane: "THE COORDINATOR HOLDS NO LANE" | One bead at a time, via `bd update --claim`; Air records it on `PostToolUse` | F `CLAUDE.md:128-131`; F `main-agent-protocol.md:75-115`; A `decisions.md` 2026-08-20 |
| Build per-worker queues (assignee, priority, `blocks` edges) | **Required**; "never answer 'what should I do next?' with an assignment" is the older adopter rule, superseded for Air by the 2026-08-20 decision | Forbidden | A `decisions.md` 2026-08-20; F `main-agent-protocol.md:65-67` |
| Hand over (`awaiting_review`, digest, close own beads with evidence) | n/a | **Required**, in order; "never print a `bd` command for the owner to run" | F `worktree-protocol.md:217-232`; A `rules/worktree-protocol.md §6` |
| Machine-level actions (installs, native builds, dev servers, leases) | Owner or coordinator arbitrates; native builds are guard-denied to the coordinator too | Forbidden | F `main-agent-protocol.md:60-73`; F `docs/log.d/2026-08-17-coordinator-round-close.md:31`; A `rules/worktree-protocol.md §2` |
| Touch another worktree's tree | Forbidden (read `git show`, never write) | Forbidden | A `rules/worktree-protocol.md §2`; nothing in adopter enforces it (§2.3) |
| Round goal and round digest | **Required**: "A round without a coordinator digest is not closed" | Architecture digest per hand-over, capped | F `main-agent-protocol.md:213-268`; F `worktree-protocol.md:217-232` |
| Rulings, arbitration, reassigning stalled work, `bd human` answers | **Required** | Escalate by filing, never by prompting | F `overnight-fleet-retrospective.md:464-467`; A `plans/0001 §6`; A `rules/worktree-protocol.md §7` |
| Messaging | Reachable; relays are the least reliable channel, so facts come from `air status` | Direct to peers by name; announce before touching shared files | F `round-2026-08-15-evening-retrospective.md:63-70`; A `rules/worktree-protocol.md §5`; A `decisions.md` 2026-08-20 |
| Verify (`cargo test`, `air record verify`) | Free, but it holds no lane so it rarely needs to | **Required** at hand-over, recorded, foreground | F `worktree-protocol.md:41-49`; A `plans/0001 §4` |
| Subagents | Allowed | Allowed | F `CLAUDE.md:151-158` |

### 2.3 What mechanically stops role drift today

- `scripts/land.sh:485-495`: `[ -d .git ] || die "land runs in the main checkout only"`, then
  `CLAUDECODE` set without `ALLOW_MAIN=1` dies with "land is the owner's command". This is the
  only hard worker-to-coordinator gate in adopter.
- `scripts/pre-commit:39-50`: refuses a commit when `.git` is a directory and `CLAUDECODE` is set.
  The only worktree-conditional deny in the guard family, and it points the other way: it stops the
  coordinator doing worker work.
- `scripts/lib/cmd-guard.py` (via `~/.claude/settings.json` PreToolUse, `lease-guard.sh:16-21`):
  every deny category is role-independent (native builds, publishing, `git push`, `bd sync`, `gh`
  writes, interactive editors) except `cargo run/test/nextest/bench`, denied only when
  `cwd == REPO` (`cmd-guard.py:486-490`). Scope is `"$REPO"|"$REPO"/*` (`lease-guard.sh:139`), so
  main and worktrees are treated identically.
- Nothing gates `bd create`, triage, digests, lease-taking, or writing to a sibling worktree. The
  `_require-worktree` target in plan 0010 (`docs/plans/0010:920-925`) was never built; the inline
  `@[ -f .git ] ||` at `Makefile:175,213` exits 0.
- Settings: `.claude/settings.json` has no `permissions` block and says so ("anything here can be
  edited by an agent working in a worktree"); `settings.local.json` carries only `env`
  (`BEADS_ACTOR`, `CARGO_TARGET_DIR`, `CARGO_BUILD_JOBS`). No `.claude/agents/`, no
  `CLAUDE.local.md` anywhere. Two of four worktrees have no `.claude/` at all and two point their
  `BEADS_ACTOR` at a different worktree's name (written by `Makefile:216-227`).
- Launch: there is no launcher. The canonical start is a human typing `claude --worktree <name>`
  and a prose prompt (`docs/plans/0010:928-933`); `fleet.sh:42,60-61` only `pgrep`s for it. No
  per-role prompt, agent, or flag is passed. Plan 0010 names the gap: the guard "constrains a
  session *already* in a worktree, it does not stop one being started in main. That is launch
  discipline" (`0010:392-395`).

### 2.4 What the retros record

Coordinator drifting into worker work, repeatedly: it dirtied main at land time with `log.md`,
four notes, two rules, the beads export and a generated file, and the owner committed the docs
three times mid-round to unblock lands (`main-agent-protocol.md:37-41`); the owner had to write
"THE COORDINATOR HOLDS NO LANE" three times in one session (`CLAUDE.md:128-131`); it asked a
worker to edit an append-only entry and the worker refused, with the lesson "the agent doing the
work is the one who must refuse" (`overnight-fleet-retrospective.md:154-159`); a round closed
with no coordinator digest (`round-2026-08-15-evening-retrospective.md:9-11,55-61`).

Workers drifting into coordinator work: no recorded case. A repo-wide search of `docs/notes/`,
`docs/log.d/` (212 files), `docs/rules/`, `docs/plans/` for landing from a worktree, committing on
main, or touching another worktree found nothing. Lane discipline is positively recorded: "no lane
reported a single conflict" across cross-merges (`round-2026-08-15-evening-retrospective.md:111-
118`), zero conflicts on 2026-08-17 (`log.d/2026-08-17-coordinator-round-close.md:19`). The same
file set also records the caution against declared-but-unenforced lanes: prompt-level file
assignment measured 55.5% against a single agent's 57.2%, "either enforce a partition
observationally or do not write the rule" (`fleet-runs.md:54-59`).

### 2.5 How Metis scopes a session (for contrast)

Metis has no coordinator/worker split. One session runs one task: `/metis-ralph SHORT_CODE`
verifies the task exists, runs `setup-metis-ralph.sh` to write
`.claude/metis-ralph-active-<session>.yaml`, and the plugin's `Stop` hook returns
`"decision": "block"` with the next-iteration prompt until the model prints
`<promise>TASK COMPLETE</promise>` or `max_iterations` is hit (`plugins/metis/commands/metis-
ralph.md:1-60`, `hooks/stop-hook.sh:27-31,75-77,228-233`, `scripts/setup-metis-ralph.sh:141-143`).
The worker never completes its own task: "Do NOT transition the task to 'completed', that requires
user approval" (`metis-ralph.md:59`). Tool restriction is light: the command's `allowed-tools`
frontmatter pre-approves only the setup script and one MCP read (`metis-ralph.md:4`), and the two
agents carry `tools:` lists (`agents/flight-levels.md:35`, `agents/code-index-summarizer.md:40`).
What Air can take: the "never close your own task" shape (Air's hand-over gate already is that),
the per-session state file keyed on `session_id`, and the Stop-hook loop as the way to keep a
headless worker on one bead.

## 3. What Claude Code can confine, per mechanism (verified against docs 2026-08-20)

Two facts decide most of this table. First, **role is a property of the checkout, not the
session**: the coordinator is the session whose cwd is the main checkout; a worker is a session
whose cwd is a linked worktree. Air already derives this (`crates/ledger/src/paths.rs:37-62`
returns `main` for the primary checkout, the directory name otherwise), and the hook input's
`cwd` is the worktree root, moving with `cd` (worktrees.md, "Hook paths don't follow the
worktree"). Second, the settings files a session reads are **not** per-worktree in the way the
control-surfaces doc assumed (`claude-code-control-surfaces.md:147` says "per-worktree"); see
rows 1 and 2.

| # | Mechanism | What it can express | Per-worktree? | Survives `--dangerously-skip-permissions`? | Agent can bypass? | Friction | Source |
|---|---|---|---|---|---|---|---|
| 1 | `.claude/settings.json` (project) | allow/deny/ask rules, hooks, `env`, `agent`, `defaultMode` | Follows the checkout: a worktree reads the copy on its branch, so all worktrees and main share one file unless a branch diverges. `EnterWorktree` "takes ... project configuration such as `CLAUDE.md` and settings to that location" | Deny rules yes ("Deny rules block in every mode, including `bypassPermissions`"); allow rules no | Can edit the file (`.claude` is a protected path only outside bypass); hooks are file-watched, so an edit is live | None at launch; one file to maintain | settings.md "Settings files"; worktrees.md "Ask Claude to create a worktree"; permission-modes.md "Available modes" |
| 2 | `.claude/settings.local.json` | same keys, uncommitted | **No.** Since v2.1.211 it is read and written "at the root of the git repository, resolved through worktrees to the main checkout, so one file covers sessions started in any subdirectory or worktree". A legacy file in the worktree dir is still read; when both set a key the root wins, and permission rules from both stay in effect | Deny yes, allow no | Same as row 1 | A deny placed here binds the coordinator too; cannot separate roles | settings.md "Settings files"; permissions.md "Permission system"; worktrees.md "Permission approvals" |
| 3 | `CLAUDE.md` / `CLAUDE.local.md` | prose | `CLAUDE.md` follows the checkout (same on every branch). `CLAUDE.local.md` is the only file that is genuinely per-worktree: "a gitignored `CLAUDE.local.md` only exists in the worktree where you created it" | n/a (advice) | Yes, by definition: "Instructions in your prompt or `CLAUDE.md` shape what Claude tries to do, but they don't change what Claude Code allows" | None | memory.md "CLAUDE.local.md"; permissions.md "Manage permissions" note |
| 4 | Launch flags: `--disallowedTools`, `--allowedTools`, `--tools`, `--settings '<json>'`, `--permission-mode`, `--setting-sources` | Deny rules (bare name removes the tool from context; scoped rule blocks matching calls), tool list, inline settings (incl. `env` and hooks) for this process only | **Yes**: flags are per process, and each worker is its own process in its own cwd | `--disallowedTools` are deny rules, so yes. `--permission-mode` is overridden by the skip flag | Not from inside the session (no file to edit). The escape is a nested `claude` run without the flags; close with `Bash(claude *)` in deny | A launcher script; adopter already has `make` targets per worktree | cli-reference.md rows `--disallowedTools`, `--tools`, `--settings`, `--permission-mode` |
| 5 | `--append-system-prompt(-file)` / `--system-prompt` | Role prose at higher priority than CLAUDE.md; `--system-prompt` replaces the default prompt entirely | Yes (per process) | n/a (advice) | Yes (advice) | None beyond the launcher | cli-reference.md "System prompt flags" |
| 6 | Custom agent as main thread: `.claude/agents/<role>.md` + `claude --agent <role>` or `agent` setting | `tools`, `disallowedTools`, `permissionMode`, `hooks`, `skills`, `initialPrompt`, `model` in one file. "the main thread itself takes on that subagent's system prompt, tool restrictions, and model". Restored on `--resume` | Yes (per process). Agent files are project-scope (`.claude/agents/`, follows the checkout) or user-scope | `disallowedTools` are deny rules, yes; `permissionMode: bypassPermissions` can be disabled by `disableBypassPermissionsMode` | `--agent` "replaces the default Claude Code system prompt entirely, the same way `--system-prompt` does", so the role file must carry everything the default prompt gave; CLAUDE.md still loads | One markdown file per role; the owner's "invoke an agent in a very specific space with very specific commands" maps onto this directly | sub-agents.md "Supported frontmatter fields", "Run the whole session as a subagent"; settings.md `agent` |
| 7 | Subagents spawned from a session (`Agent` tool, `--agents` JSON, `isolation: worktree`) | A coordinator can fan out a confined subagent per task; each gets a temp worktree, a restricted tool list, and `maxTurns` | Yes, by construction | Parent mode applies; deny rules still bind | Parent decides | Replaces long-lived worker sessions with short ones; loses SendMessage peers and the human's per-worker terminal | sub-agents.md; worktrees.md "Isolate subagents with worktrees" |
| 8 | Built-in worktree isolation (any session started with `--worktree`, `EnterWorktree`, or resumed into one) | Blocks `Edit`/`Write`/`NotebookEdit` into the main checkout; blocks Bash whose cwd resolves to the main checkout or cannot be verified; blocks `git -C`/`--git-dir`/`GIT_DIR`/`cd` redirects into main; refuses shell shapes it cannot trace (brace expansion, unquoted heredocs). "You can't turn this check off" | **Yes**: this is the only mechanism that is per-worktree by design | Documented as unconditional; not stated mode by mode, so verify with a probe (§6) | Not documented; the command-shape check exists precisely to close rewrite escapes | Real: some multi-line Bash gets refused and must be split; Air's own `air record verify -- <cmd>` style invocations should stay single-command | worktrees.md "How Claude Code enforces isolation" |
| 9 | PreToolUse hook (`air hook`) with exit 2 / `permissionDecision: "deny"` | Arbitrary logic on `cwd`, `tool_input.command`, `permission_mode`, session id; Air already parses `bd` commands here (`crates/cli/src/cmd/hook.rs:286-313`) | Yes, because the hook sees `cwd` and can derive the role (`paths.rs:37-62`) | Hooks receive `permission_mode: "bypassPermissions"` as an input value, so they run in that mode; exit 2 is "the one outcome JSON can't override". A hook block also beats allow rules. Deny rules beat a hook allow | The hook binary path is fixed (`air install` resolves it, plan 0001 §5). `disableAllHooks` in a settings file or `--settings` turns all hooks off unless managed; `.claude/settings.json` can be edited by the agent in bypass mode. The `if` filter is best-effort: "use the permission system rather than a hook to enforce a hard allow or deny" for hard guarantees | ~100 ms per tool call, already paid | hooks.md "Exit code 2", "Disable or remove hooks", "Bash `if` matching", common input `permission_mode`; permissions.md "Extend permissions with hooks" |
| 10 | `permissions.deny` Bash patterns | `Bash(air land *)`, `Bash(bd create *)`, `Bash(git push *)`, `Bash(claude *)`; wildcard spans spaces; deny matches past leading `VAR=`; each subcommand of `&&`, `;`, `|` is matched separately; a fixed wrapper list (`timeout`, `nice`, `nohup`, ...) is stripped | Only via rows 1, 2, 4, 6 | Yes | Environment runners (`mise exec`, `npx`, `docker exec`) are not stripped; `bash -c "..."` and scripts are opaque; args-constraining patterns are "fragile" per the docs | None | permissions.md "Bash", "Compound commands", "Wrappers" |
| 11 | `Edit(path)` / `Read(path)` deny rules | Block file tools and recognised file commands (`cat`, `sed`, ...) on a path; `Read` deny also blocks Edit/Write there | Path rules in local settings anchor at the start directory, so `Edit(/src/**)` matches each worktree's own `src/` | Yes | "They don't apply to arbitrary subprocesses that read or write files indirectly" (a script, `cargo`, `python`) | None | permissions.md "Read and Edit"; "Local settings rules anchor at the directory you started" |
| 12 | Bash sandbox (`sandbox.enabled`, `filesystem.allowWrite/denyWrite`) | OS-level (seatbelt/bubblewrap) write confinement to cwd + tmp; in a linked worktree it also allows writes to the shared `.git` except `hooks/` and `config` | Effectively yes (cwd-based) | Independent of permission mode ("`/sandbox` is not a permission mode") | Commands that need to write elsewhere fail and must be allow-listed; `unsandboxed` escape exists unless `allowUnsandboxedCommands: false` | Setup; cargo target dirs must be inside the worktree (they are, per protocol §3) | sandboxing.md "Filesystem isolation", "Permission modes" |
| 13 | Managed settings (`/Library/Application Support/ClaudeCode/managed-settings.json`) | Same keys, unoverridable; `disableBypassPermissionsMode`, `allowManagedHooksOnly`, `disableSideloadFlags` | No (machine-wide) | Yes, by definition | No | Machine-level install; shared with the adopter fleet on the same machine, so a mistake hits it too | settings.md "Managed settings"; permissions.md "Managed-only settings" |
| 14 | Plugins | Bundle agents, hooks, skills; no permission rules of their own | No | Hooks yes (as row 9) | Can be disabled unless managed | Packaging work | plugins.md |
| 15 | Env var (`AIR_ROLE`) via `settings.env` or the launcher | A label the hook and prose can read | Only via the launcher (row 4) or `--settings` | n/a | Trivially (`AIR_ROLE=coordinator bd create ...`); leading assignments are stripped for matching but not for the process | None | settings.md `env`; permissions.md "Wrappers" |

### 3.1 Corrections to `claude-code-control-surfaces.md`

- `:147` lists project settings as "per-worktree". A worktree reads the `.claude/settings.json`
  on its own branch, which is the same file on every branch until someone commits a change, and
  `settings.local.json` resolves to the main checkout (settings.md "Settings files"). Neither is a
  per-role surface.
- `:192` treats `--dangerously-skip-permissions` as "everything". Deny rules, ask rules, hooks,
  critical-path `rm`, and the built-in worktree isolation all still apply (permission-modes.md
  "Available modes", "Actions no mode auto-approves"; worktrees.md "How Claude Code enforces
  isolation").
- The worktree isolation checks (row 8) and the `EnterWorktree` approval prompt are not in the
  doc at all.

## 4. Evaluating the owner's ideal: "invoke an agent in a very specific space with very specific commands"

Claude Code offers that shape directly, with one caveat each:

- **Specific space.** `claude --worktree <name>` is the space, and the isolation in row 8 is the
  fence: no edits, commands, or git redirects into main, unconditionally. It does not fence
  sibling worktrees explicitly (they live under main's `.claude/worktrees/`, and the docs only
  say "a path in the main checkout"; §6 has the probe). Nothing in adopter's record needs that
  fence yet (§2.4).
- **Specific commands.** `--disallowedTools "Bash(air land *)" "Bash(bd create *)" ...` at launch
  gives deny rules the session cannot edit away and that hold in bypass mode (row 4). A bare tool
  name removes the tool from context, which is how to take `EnterWorktree`/`ExitWorktree` away
  from a worker. The caveat is the documented fragility of argument patterns (row 10): `bd create`
  behind `bash -c`, `mise exec`, or a script is invisible to the matcher. The hook (row 9) is the
  second layer for the shapes the matcher cannot see, and `AIR_ENFORCE=1` is its switch.
- **Specific role prompt.** `--append-system-prompt-file docs/rules/roles.md` puts the role text
  above CLAUDE.md without replacing Claude Code's default prompt. The `--agent worker` form bundles
  prompt + `disallowedTools` + `permissionMode` + `hooks` in one `.claude/agents/worker.md`, is
  restored on `--resume`, and shows `@worker` in the header, but it replaces the default system
  prompt entirely (row 6). That is a real cost for a general coding worker and the reason §5
  prefers the append form until a probe shows the agent form performs as well.
- **Launcher.** All of this lives in a launcher, which is the thing adopter never built (§2.3).
  `air worker <name>` (or `air coordinator`) assembles the flags; it is the cheapest place to make
  role mechanical because it is the only place that knows the role before the first tool call.

Against "stay CLAUDE.md-driven": the docs are explicit that prompt text "shape[s] what Claude
tries to do, but [doesn't] change what Claude Code allows" (permissions.md "Manage permissions"),
adopter measured prompt-level lane rules below no rule at all (`fleet-runs.md:54-59`), and rules
had to be written three times before they bound (`CLAUDE.md:128-131`). CLAUDE.md stays the place
for the *why* and the hand-over order; it is not the place for the deny list.

## 5. Recommended layered design for Air

Ordered by cost and by what the record already justifies. Each layer ships with a red/green
probe (§6), per `CLAUDE.md` "Only build what makes sense".

| Layer | Enforced by | What | Justified by |
|---|---|---|---|
| L0 | Claude Code, already on | Worker cannot write to, run in, or point git at the main checkout (row 8). Air does nothing except start workers with `--worktree` (or `EnterWorktree`) and never by `cd`. | Free; protects the coordinator's clean main, which is the recorded pain (§2.4) |
| L1 | `air worker <name>` launcher (row 4, 5) | `claude --worktree <name> --append-system-prompt-file docs/rules/roles.md --disallowedTools "Bash(air land *)" "Bash(git push *)" "Bash(bd create *)" "Bash(bd sync *)" "Bash(claude *)" "EnterWorktree" "ExitWorktree" --settings '{"env":{"AIR_ROLE":"worker","BEADS_ACTOR":"<name>","CARGO_TARGET_DIR":"...","CARGO_BUILD_JOBS":"..."}}'`. Replaces adopter's `worktree-env` file-writing (and its drift, §2.3) with per-process values. `air coordinator` does the mirror: `--disallowedTools "Bash(git commit *)" "Bash(git add *)" "Bash(cargo run *)"` plus the coordinator prose. | Deny rules hold in every mode; no file an agent can edit; `BEADS_ACTOR` drift on 2026-08-14 |
| L2 | `air hook` PreToolUse (row 9) | Role derived from `cwd` (`paths.rs:37-62`), recorded as `role` on the `sessions` row and on the event line. A per-role deny list for the command shapes L1's matcher misses (`bash -c`, `mise exec`, scripts), using the same tokeniser as `is_handover_command`. Advisory (`additionalContext` naming the rule and the fixing command) until `AIR_ENFORCE=1`, matching the hand-over gate's rollout. Add the coordinator-side check: `Edit|Write` under the main checkout by a `main`-role session while any worker session is live, warn. | Consistent with plan 0001 §5; adopter's guard-edited-in-a-worktree lesson (`lease-guard.sh:41-45`) is already covered by `air install` resolving the binary path |
| L3 | `docs/rules/roles.md` prose + CLAUDE.md index | The *why*, the hand-over order, the capture-not-file rule, and what to do when denied. Marked `[Air enforces]`/`[Air advises]`/`[prose]`. | Rules in the right register (`adopter-as-built.md §4.5`) |
| Not now | `--agent worker` (row 6), Bash sandbox (row 12), managed settings (row 13), subagent-per-task fan-out (row 7) | Each is a larger change in how a worker behaves or a machine-wide install shared with the live adopter fleet. | No named pain in the record; revisit after one advisory round |

What stays prose forever: which bead to pick, what counts as "outside my bead", when to escalate,
the digest content, the coordinator's triage judgement (plan 0002 §5).

## 6. Dialectic

**The case that 1 + 3 already works and confinement is over-engineering.** The owner reports the
lanes hold. The retros agree: zero worker-side drift in 212 log entries, zero cross-merge
conflicts on two recorded rounds (§2.4). The only hard gate adopter has (`land.sh`) has never
been recorded as tripping on a worker. Gas Town is the warning that roles and fences built ahead
of a measured trigger become the product (`docs/decisions.md` 2026-08-18). Every deny rule is also
a way to be wrong: the docs themselves call argument patterns fragile, adopter's guard produced a
214-route false alarm and a silent no-op fix with 106 green probes (`adopter-as-built.md §4.6`),
and a worker denied `bd create` by a pattern that also catches `bd create --help` loses a turn for
nothing. On this reading the right move is L0 (free) and L3 (prose), and nothing else until a
worker is caught doing coordinator work.

**Assessment.** The steelman holds for the worker side, and §5 reflects it: L1 and L2 are cheap
and reversible, and L2 is advisory, so the first round produces counts rather than refusals. It
does not hold for the coordinator side, where the record is unambiguous and repeated (dirty main,
three restatements of "holds no lane", a missing digest). The asymmetry is the finding: the
recorded pain is coordinator drift, and the mechanisms most people reach for (deny lists for
workers) do not touch it. The coordinator-side items in L1/L2 (`git commit` denied in main for an
agent session, a warning on main-checkout edits while workers are live) are the ones the record
already pays for. adopter's `pre-commit:39-50` does the first half today, and Air should keep
it rather than rebuild it.

**Evidence that would justify each layer.** L1 worker denies: an event line showing a worker
session ran `air land`, `git push`, or `bd create` (the advisory L2 hook produces exactly this
count for free). L2 enforcement (`AIR_ENFORCE=1`): two advisory rounds with at least one
would-refuse event per round and no false positive. `--agent worker`: a side-by-side round where
the append-prompt worker and the agent-form worker land comparable work, measured by
`landings` and `verify_runs`. Sandbox: any recorded write outside the worktree by a subprocess
that `Edit` rules cannot see. Sibling-worktree fence: any recorded cross-worktree write.

**What would falsify §5.** If the advisory `role` events show zero would-refuse events over
several rounds while coordinator drift persists, L1's worker deny list should be dropped and the
effort moved to the coordinator side entirely.

## 7. Probes to ship with each layer

1. L0: from a `--worktree` session, `Edit` a file under the main checkout path and run
   `git -C <main> status`; expect both refused. Then the sibling question: `Edit` a file under
   `<main>/.claude/worktrees/<other>/`; record whether it is refused (the docs do not say).
2. L1: `claude -p --worktree probe --disallowedTools "Bash(air land *)" --dangerously-skip-permissions "run: air land x"`;
   expect a deny. Same with `bash -c "air land x"`; expect it to pass the matcher (documenting the
   gap L2 fills).
3. L2: `air selftest` extended with a `main`-cwd and a worktree-cwd PreToolUse fixture for each
   deny shape; one event line each with `role` and `decision`.
4. Coordinator: with a live worker session row, `Edit` in main from a `main` session; expect the
   advisory warning.

## 8. Open questions for the owner

1. Should `air coordinator` exist as a launcher, or is "a session in the main checkout" enough of a
   definition, with L2 deriving the role? (§5 assumes the latter works and the launcher is a
   convenience.)
2. `bd create` from a worker: the 2026-08-18 decision says workers capture and do not file. Is
   that a deny (L1) or an advisory count (L2 only)? adopter never gated it.
3. Is the sibling-worktree fence wanted at all, given no recorded incident? Probe 1 answers whether
   Claude Code already provides it.
4. Does a headless worker (`claude -p --worktree`, Metis-style Stop-hook loop, §2.5) belong in the
   next increment, or do workers stay interactive terminals the owner can watch?
5. `CLAUDE.local.md` is the only genuinely per-worktree file Claude Code reads. Worth using for the
   worker's name and lane, or is `--append-system-prompt-file` enough?

## Sources

Official docs, all fetched 2026-08-20 as `https://code.claude.com/docs/en/<page>.md`:
`settings` (Settings files; Worktree settings; `agent`, `env`, `disableAllHooks`,
`disableSideloadFlags`), `permissions` (Permission system; Manage permissions; Bash; Compound
commands; Wrappers; Read and Edit; Extend permissions with hooks; Working directories; Managed-only
settings), `permission-modes` (Available modes; Actions no mode auto-approves; Common setups;
Skip all checks with bypassPermissions mode; Protected paths), `hooks` (Configure hooks, scope
table; Bash `if` matching; Reference scripts by path; Disable or remove hooks; common input
`permission_mode`; Exit code 2), `worktrees` (Start Claude in a worktree; Ask Claude to create a
worktree; How Claude Code enforces isolation; Isolate subagents with worktrees; Permission
approvals), `sub-agents` (Supported frontmatter fields; Invoke subagents explicitly; Run the whole
session as a subagent), `cli-reference` (`--agent`, `--agents`, `--allowedTools`,
`--disallowedTools`, `--tools`, `--settings`, `--setting-sources`, `--permission-mode`,
`--dangerously-skip-permissions`; System prompt flags), `memory` (CLAUDE.local.md), `sandboxing`
(Filesystem isolation; Permission modes; `allowUnsandboxedCommands`), `plugins`.

adopter (`~/projects/adopter` @ `71191e0`, read only): `CLAUDE.md:47,100-104,
128-131,151-158,218-225`; `docs/rules/worktree-protocol.md:11-49,208-232`;
`docs/rules/main-agent-protocol.md:3-41,60-115,213-268`; `docs/rules/fleet-runs.md:22-32,54-59`;
`docs/plans/0010-multi-agent-parallel-work.md:392-395,920-933`; `scripts/land.sh:485-524`;
`scripts/pre-commit:35-92`; `scripts/lease-guard.sh:16-21,120-139`;
`scripts/lib/cmd-guard.py:63-68,82-84,300-333,389-529`; `scripts/lease.sh:19-20,67-74`;
`scripts/fleet.sh:42,60-61,123-126`; `Makefile:164-227,378-390,424-437,457-470`;
`.claude/settings.json`, `.claude/settings.local.json`, `.claude/worktrees/*/.claude/settings*.json`;
`~/.claude/settings.json` (PreToolUse entry, `autoMode.soft_deny`);
`docs/notes/overnight-fleet-retrospective.md:147-159,464-476`;
`docs/notes/round-2026-08-15-evening-retrospective.md:9-11,55-70,111-118`;
`docs/log.d/2026-08-17-coordinator-round-close.md:19-31`.

Metis (`~/projects/metis` @ `6745810`, read only): `plugins/metis/commands/metis-
ralph.md:1-60`, `commands/metis-ralph-tasks.md:61`, `hooks/hooks.json`, `hooks/stop-hook.sh:27-
31,75-77,228-233`, `scripts/setup-metis-ralph.sh:141-143`, `agents/flight-levels.md:33-35`,
`agents/code-index-summarizer.md:38-40`.

Air: `CLAUDE.md`; `docs/research/claude-code-control-surfaces.md:141-165,442-520,580-620,855-
900`; `docs/rules/worktree-protocol.md`; `docs/plans/0001-first-slice.md §4-7`;
`docs/plans/0002-what-to-work-on.md §5`; `docs/decisions.md` (2026-08-18, 2026-08-20);
`docs/research/adopter-as-built.md §2.2-2.11, §4.5-4.7`;
`docs/research/adopter-enforcement-and-skills.md §0, §1.5-1.6`; `crates/ledger/src/paths.rs:9-62`;
`crates/ledger/src/schema.rs:50-58`; `crates/cli/src/cmd/hook.rs:93-102,241-330`;
`.claude/settings.json`.
