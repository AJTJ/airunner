# Claude Code facts Air relies on

This is the one reference for what Claude Code itself provides, so that Air does not rebuild it and a claim about the harness has a place to be checked. On 2026-09-14 it condensed three documents that were then deleted: `docs/research/claude-code-control-surfaces.md` (§0 inventory, §1 hook events), `docs/research/agent-roles-and-confinement.md` (§3 confinement table, §3.1 corrections) and `docs/research/verification/ticks/2026-08-18-0245-claude-code-hook-edge-cases.md` (findings table). Every fact keeps the date it was checked and the page or line it came from; nothing was re-verified on 2026-09-14 except the installed version.

## 1. Inventory of what the harness ships

Checked 2026-08-24 against Claude Code 2.1.241, first-hand from a running session. `claude --version` on 2026-09-14 reports 2.1.272. By this file's own rule (re-run after a release, or after a month) the inventory is stale: the version has moved from 2.1.241 to 2.1.272 and it has not been re-run.

### 1.1 Launch and session control (`claude --help`, 2026-08-24)

| Area | Flags |
|---|---|
| Isolation | `-w, --worktree [name]`; `--tmux` (needs `--worktree`; `--tmux=classic` for plain tmux); `--add-dir`; `--teleport`, `--cloud`, `--environment`, `--remote-control` |
| Identity | `--agent <name>`, `--agents <json>`, `--system-prompt`, `--append-system-prompt` (and `-file` variants), `-n, --name`, `--session-id`, `-r/--resume`, `-c/--continue`, `--fork-session`, `--from-pr` |
| Permission | `--permission-mode`, `--allowed-tools`, `--disallowed-tools`, `--tools`, `--settings`, `--setting-sources`, `--safe-mode`, `--disable-slash-commands`, `--dangerously-skip-permissions` plus the two-step `--allow-dangerously-skip-permissions` |
| Budget and model | `--max-budget-usd` (hard spend ceiling), `--model`, `--fallback-model`, `--effort`, `--autocompact` |
| Machine-readable | `-p/--print`, `--output-format` (incl. `stream-json`), `--input-format`, `--json-schema`, `--include-partial-messages`, `--include-hook-events`, `--replay-user-messages`, `--forward-subagent-text`, `--verbose`, `--debug-file` |
| Background | `--bg`, managed with `claude agents` (`--cwd`, `--json`, `--all`, `--agent`, `--add-dir`) |
| Extension | `--mcp-config`, `--strict-mcp-config`, `--plugin-dir`, `--plugin-url`, `--bare` |
| Subcommands | `agents`, `auth`, `auto-mode`, `doctor`, `gateway`, `import`, `install`, `mcp`, `plugin`, `project` (with `purge`), `setup-token`, `ultrareview`, `update` |

### 1.2 In-session tools (observed 2026-08-24)

Orchestration: `Agent` (with `subagent_type: "fork"`, which inherits the caller's context, and `isolation: "worktree"`), `Workflow`, `TaskOutput`, `TaskStop`, `ListAgents`. Messaging: `SendMessage` reaches a subagent, another local session or a cloud session; `notify_when_idle: true` delivers one notice when a named local session next goes idle or exits, with the instruction never to poll `ListAgents` instead. Waiting: `Monitor` (each stdout line of a script becomes a notification), `Bash` with `run_in_background`, `CronCreate`/`CronList`/`CronDelete`, `ScheduleWakeup`, `PushNotification`. Workspace: `EnterWorktree`/`ExitWorktree`, plan mode, `LSP`, the file tools, `Bash`, `WebSearch`/`WebFetch`, `ToolSearch`, `Skill`, `AskUserQuestion`, `SendUserFile`, `ReportFindings`, `Artifact`, MCP resource readers.

### 1.3 The limits that matter to Air (2026-08-24)

`CronCreate` is session-only: jobs live in memory, die with the session, fire only while the REPL is idle, and recurring jobs expire after seven days. It replaces in-session polling, not an external cron that restarts a coordinator.

`Monitor` and `notify_when_idle` do replace polling. Air's `idle-without-claim` condition polled the ledger and fired 25,958 times over two subjects between 2026-08-15 and 2026-08-24 (`air audit --since 2026-08-15`, run 2026-08-24). A one-shot idle subscription per worker is the first-party form of the same fact.

Roles, deny lists and env do not need a launcher: `--agent`, `--append-system-prompt`, `--disallowed-tools`, `--settings` and an `env` block cover what `air worker` assembles. The only thing without a first-party equivalent is the detached start used when the coordinator has no tty. `--max-budget-usd` exists, so an Air spend cap should be a flag, not code.

Not checked in that pass: new hook event types after 2026-08-17, the plugin and gateway surfaces, `auto-mode`, `import`. When a limit above stops being true, the Air mechanism built on it is a deletion candidate that week.

## 2. Hook events

Source: https://code.claude.com/docs/en/hooks.md, fetched 2026-08-17 (Claude Code 2.1.234). Exit code 2 from a command hook blocks regardless of JSON; `permissionDecision: "deny"` blocks on PreToolUse and PermissionRequest; `continue: false` stops processing. Default timeouts: 600 s for command/http/mcp_tool (30 s on UserPromptSubmit), 30 s prompt, 60 s agent; SessionEnd hooks share 1.5 s, raisable to 60 s aggregate by a hook's own `timeout`.

| Event | Fires | Blocks | Modifies |
|---|---|---|---|
| SessionStart | session begins or resumes | yes | none |
| SessionEnd | session ends | no | none |
| Setup | `--init-only` or maintenance | yes | none |
| UserPromptSubmit | prompt submitted | yes | `updatedInput` |
| PreToolUse | before a tool runs | yes (exit 2) | `permissionDecision`, `updatedInput` |
| PostToolUse / PostToolUseFailure | tool succeeded / failed | no | context only |
| PostToolBatch | parallel batch resolved | yes | `blockAgentic` |
| Stop | turn ends | see §4 | none |
| StopFailure | turn ends on API error | no | none |
| PermissionRequest | permission prompt needed | yes | `decision` allow/deny/escalate |
| PermissionDenied | permission denied | yes | `retry: true` |
| SubagentStart / SubagentStop | subagent spawned / done | yes / no | none |
| TaskCreated / TaskCompleted | task spawned / done | yes / no | none |
| Notification, MessageDisplay | UI events | no | none |
| PreCompact / PostCompact | compaction | yes / no | none |
| CwdChanged, DirectoryAdded, FileChanged | cwd or file changes | no | none |
| WorktreeCreate / WorktreeRemove | worktree lifecycle | yes | none |
| ConfigChange, InstructionsLoaded | config or CLAUDE.md loaded | no | none |
| Elicitation / ElicitationResult | MCP form prompt / response | yes / no | varies |
| UserPromptExpansion | slash command expansion | yes | `updatedPrompt` |

The 2026-08-17 table had Stop as non-blocking; the 2026-08-18 check in §4 found exit 2 on Stop does block, up to a cap. §4 wins.

## 3. What Claude Code can confine a worktree session to

Verified against the docs on 2026-08-20 (`https://code.claude.com/docs/en/<page>.md`, fetched that day). Two facts decide most rows. Role is a property of the checkout: the coordinator's cwd is the main checkout, a worker's is a linked worktree, and Air derives this in `crates/ledger/src/paths.rs:56` (`worker_name_for`). And the settings files a session reads are not per-worktree (rows 1, 2).

| # | Mechanism | Per-worktree | Holds in bypass mode | Escape or cost | Source |
|---|---|---|---|---|---|
| 1 | `.claude/settings.json`: allow/deny/ask, hooks, `env`, `agent` | Follows the branch; shared unless a branch diverges | Deny yes ("Deny rules block in every mode, including `bypassPermissions`"); allow no | Agent can edit it, and hooks are file-watched | settings.md "Settings files"; permission-modes.md "Available modes" |
| 2 | `.claude/settings.local.json` | No. Since v2.1.211 it resolves through worktrees to the main checkout; a legacy worktree copy is still read, root wins | Deny yes, allow no | A deny here binds the coordinator too | settings.md "Settings files"; worktrees.md "Permission approvals" |
| 3 | `CLAUDE.md` / `CLAUDE.local.md` | `CLAUDE.local.md` is the only genuinely per-worktree file | Advice only: prompt text "shape[s] what Claude tries to do, but [doesn't] change what Claude Code allows" | n/a | memory.md; permissions.md "Manage permissions" |
| 4 | Launch flags `--disallowedTools`, `--allowedTools`, `--tools`, `--settings '<json>'`, `--permission-mode` | Yes, per process | `--disallowedTools` are deny rules, yes; `--permission-mode` is overridden by the skip flag | A nested `claude` without the flags; deny `Bash(claude *)` | cli-reference.md |
| 5 | `--append-system-prompt(-file)` / `--system-prompt` | Yes | Advice only | `--system-prompt` replaces the default prompt | cli-reference.md "System prompt flags" |
| 6 | `.claude/agents/<role>.md` with `claude --agent <role>`: `tools`, `disallowedTools`, `permissionMode`, `hooks`, `model` | Yes, per process; restored on `--resume` | `disallowedTools` yes | Replaces the default system prompt entirely | sub-agents.md "Run the whole session as a subagent" |
| 7 | Subagents (`Agent`, `--agents`, `isolation: worktree`) | Yes | Parent mode applies; deny rules bind | Loses SendMessage peers and the owner's terminal per worker | sub-agents.md; worktrees.md "Isolate subagents with worktrees" |
| 8 | Built-in worktree isolation (`--worktree`, `EnterWorktree`): blocks Edit/Write into main, Bash whose cwd resolves to main or cannot be verified, `git -C`/`--git-dir`/`GIT_DIR`/`cd` into main, untraceable shell shapes | Yes, by design | "You can't turn this check off"; not stated mode by mode | Some multi-line Bash is refused and must be split | worktrees.md "How Claude Code enforces isolation" |
| 9 | PreToolUse hook (`air hook`) exit 2 or `permissionDecision: "deny"`; Air parses `bd` here (`crates/cli/src/cmd/hook.rs:851`) | Yes, the hook sees `cwd` | Hooks run in bypass mode; exit 2 is "the one outcome JSON can't override" | `disableAllHooks` in any settings file turns hooks off unless managed; docs say use permissions for a hard guarantee | hooks.md "Exit code 2", "Disable or remove hooks"; permissions.md "Extend permissions with hooks" |
| 10 | `permissions.deny` Bash patterns: wildcard spans spaces; each `&&`/`;`/pipe segment matched separately; `timeout`, `nice`, `nohup` stripped | Only via rows 1, 2, 4, 6 | Yes | `mise exec`, `npx`, `docker exec` not stripped; `bash -c` and scripts opaque; argument patterns called "fragile" | permissions.md "Bash", "Compound commands", "Wrappers" |
| 11 | `Edit(path)` / `Read(path)` deny | Local rules anchor at the start directory | Yes | "They don't apply to arbitrary subprocesses" | permissions.md "Read and Edit" |
| 12 | Bash sandbox (`sandbox.enabled`, `filesystem.allowWrite/denyWrite`): OS-level writes to cwd, tmp, and the shared `.git` minus `hooks/` and `config` | Effectively yes | Independent of permission mode | `unsandboxed` escape unless `allowUnsandboxedCommands: false` | sandboxing.md "Filesystem isolation" |
| 13 | Managed settings; `disableBypassPermissionsMode`, `allowManagedHooksOnly` | No, machine-wide | Yes | Shared with every fleet on the machine | settings.md "Managed settings" |
| 14 | Plugins: agents, hooks, skills, no permission rules | No | Hooks as row 9 | Disableable unless managed | plugins.md |
| 15 | Env var (`AIR_ROLE`) via `settings.env` or launcher | Only via the launcher | n/a | `AIR_ROLE=coordinator bd create` (leading assignments stripped for matching, not for the process) | settings.md `env`; permissions.md "Wrappers" |

### 3.1 Corrections

The 2026-08-17 control-surfaces document got three things wrong, found 2026-08-20. It called project settings per-worktree; a worktree reads the `.claude/settings.json` on its own branch, and `settings.local.json` resolves to main (settings.md "Settings files"). It treated `--dangerously-skip-permissions` as "everything"; deny rules, ask rules, hooks, critical-path `rm` and worktree isolation all still apply (permission-modes.md "Available modes", "Actions no mode auto-approves"; worktrees.md "How Claude Code enforces isolation"). And it omitted the worktree isolation checks (row 8) and the `EnterWorktree` approval prompt.

One correction to the 2026-08-20 design (owner ruling 2026-08-29, air-iy1): denying `git commit` to the coordinator shipped and was reversed. The coordinator may commit and merge on main; only `git push` is denied, because the boundary is the remote, not main. The live list is `COORDINATOR_DENY` at `crates/cli/src/cmd/launch.rs:83`, not any argv quoted in research.

## 4. Hook edge cases

Checked 2026-08-18 against Claude Code 2.1.224+, from https://code.claude.com/docs/en/hooks-guide.md unless another source is named.

| Question | Finding | Source |
|---|---|---|
| SessionEnd on SIGKILL, crash, terminal close? | Not guaranteed; issues report the hook killed before completion on abnormal exit. Fires on normal exit (`reason: prompt_input_exit`); on an API error StopFailure fires instead. Anything relying on it needs a fallback such as transcript mtime or a sweep at next start. | github.com/anthropics/claude-code/issues/41577 and /62987, WebSearch 2026-08-18; partial |
| SessionEnd reasons | `clear`, `resume`, `logout`, `prompt_input_exit`, `other` | hooks-guide.md line 673 |
| Can PreCompact save state? | It can block (exit 2; matchers `manual`, `auto`) but runs too late to prevent summary loss. Capture holdings and claims in Stop instead. | hooks-guide.md line 676 |
| Stop hook | End of turn, not session end. Exit 2 forces Claude to continue, up to 8 blocks per turn before override (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` raises it). Input carries `stop_hook_active: true` once the hook has blocked this turn; exit 0 then. | hooks-guide.md lines 487, 988–994 |
| AskUserQuestion hook? | None. `Elicitation` covers MCP forms only; no hook can block on or answer a question, and none reports "waiting on the user". | hooks-guide.md lines 469–501 |
| Missing or failing hook | Fail-open: timeout, non-zero exit other than 2, or invalid JSON shows a hook error and the action proceeds. | hooks-guide.md lines 591–594 |
| Concurrency | Matching hooks run in parallel; on PreToolUse the most restrictive decision wins (deny > defer > ask > allow). | hooks-guide.md lines 467, 510–541 |

## Refresh

Re-run the inventory after any Claude Code release that mentions orchestration, tasks, permissions or hooks, and at least monthly. Five commands, five minutes: `claude --version`; `claude --help`; `claude <subcommand> --help` for each subcommand in §1.1; the session's own tool list and skill list for §1.2; `.claude/settings.json` in this repo for the hook events actually wired. Update the date and version at the top of §1, mark anything not observed directly as "not checked", and re-fetch the docs pages named in §2 to §4 only when a row is suspected wrong. If a §1.3 limit stops being true, the Air mechanism that depends on it is a deletion candidate the same week.
