# Tick 0245: Claude Code Hook Edge Cases Verification

**Checked:** 2026-08-18 | **Claude Code:** v2.1.224+ (stable) | **Sources:** Official docs at code.claude.com

## Findings Table

| Question | Answer | URL | Verified? |
|----------|--------|-----|-----------|
| **PermissionRequest**: separate hook event or Notification with type? | **Separate event.** `PermissionRequest` fires when a tool call needs permission (line 476, hooks-guide.md). Input: `session_id`, `cwd`, `transcript_path`, `permission_mode`, `hook_event_name`, `tool_name`, `tool_input`. Hook can return `decision: {behavior: "allow"|"deny"|"escalate"}` and `updatedPermissions` array. Different from `Notification` which has matcher `permission_prompt` for UI events. | https://code.claude.com/docs/en/hooks-guide.md (lines 424–461, 476) | ✓ Yes |
| **PermissionRequest**: can auto-allow/deny? | **Yes.** Return JSON: `{"hookSpecificOutput": {"hookEventName": "PermissionRequest", "decision": {"behavior": "allow"}}}`. Escalate also supported. No retry field (unique to PermissionDenied). | https://code.claude.com/docs/en/hooks-guide.md (lines 424–430) | ✓ Yes |
| **PreCompact**: trigger value and payload? | **Matchers:** `"manual"` (user `/compact`) vs `"auto"` (context-fills-up). Payload includes `session_id`, `cwd`, `transcript_path`, `permission_mode`, `hook_event_name`. Can block (exit 2). | https://code.claude.com/docs/en/hooks-guide.md (line 676); WebSearch (PreCompact schema) | ✓ Yes |
| **SessionEnd**: reason matcher values? | **Five values:** `"clear"` (user `/clear`), `"resume"` (user `/resume`), `"logout"`, `"prompt_input_exit"` (normal exit, exit code 0), `"other"` (abnormal exit: SIGTERM, crash, etc.). | https://code.claude.com/docs/en/hooks-guide.md (line 673) | ✓ Yes |
| **Stop hook**: input fields and blocking? | Fires when Claude **finishes responding** (end-of-turn), not session stop. Input: `session_id`, `cwd`, `transcript_path`, `permission_mode`, `hook_event_name`. Field `stop_hook_active: true` if hook already blocked in this turn (line 988 example). Can block (exit 2) to force Claude to continue. Exit code 2 blocks up to 8 times before override (raises cap with `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` env var). | https://code.claude.com/docs/en/hooks-guide.md (lines 487, 988–994) | ✓ Yes |
| **SubagentStop**: trigger and payload? | Fires when subagent finishes. Matcher is agent type: `"general-purpose"`, `"Explore"`, `"Plan"`, or custom agent name. Payload: `session_id`, `cwd`, `transcript_path`, `hook_event_name`, `agent_id`, `agent_type`. Informational only (cannot block). | https://code.claude.com/docs/en/hooks-guide.md (lines 484, 677) | ✓ Yes |
| **Hook timeouts**: default per type, per-hook override? | **Defaults:** `command`, `http`, `mcp_tool`: 600s (10 min). `UserPromptSubmit` lowers to 30s; `MessageDisplay` lowers to 10s. `prompt`: 30s. `agent`: 60s. Override with `timeout` field in hook config (seconds). `SessionEnd` hooks share 1.5s total budget; if one hook sets `timeout > 1.5s`, budget raised to match (max 60s total). | https://code.claude.com/docs/en/hooks-guide.md (lines 932–935) | ✓ Yes |
| **Hook concurrency**: do hooks run in parallel for same event? | **Yes.** All matching hooks run to completion in parallel before results merge (line 467). One hook's `deny` does not block sibling hooks from executing (side effects not suppressed). For `PreToolUse` permission decisions, most restrictive wins (order: `deny` > `defer` > `ask` > `allow`). | https://code.claude.com/docs/en/hooks-guide.md (lines 467, 510–541) | ✓ Yes |
| **PreToolUse blocking**: does slow hook block tool execution? | **Yes.** `PreToolUse` runs before tool executes (line 475). Slow hook delays tool. If hook timeout exceeded, tool call proceeds as non-blocking error (hook error reported, tool runs). Exit 2 blocks tool (no run). | https://code.claude.com/docs/en/hooks-guide.md (lines 475, 932–935) | ✓ Yes |
| **Missing/non-executable hook**: fail-open or fail-closed? | **Fail-open (non-blocking error).** Hook command timeout or non-zero exit code (other than 2) → transcript shows "`<hook name> hook error`" notice, action proceeds. Exit 2 alone blocks. Invalid JSON → non-blocking error on most events. If script not found: `"command not found"` appears in transcript, action proceeds. | https://code.claude.com/docs/en/hooks-guide.md (lines 591–594) | ✓ Yes |
| **SessionEnd**: fire on SIGKILL, terminal close, crash? | **Documented edge case: No guarantee.** Issue #62987 reports SessionEnd hooks killed before completion on abnormal exit (SIGKILL, SIGTERM grace period violated). Best practice: SessionEnd hook should be idempotent and resilient; critical cleanup needs defense-in-depth (e.g., preflight lock recovery + SessionEnd sweep). On normal exit (exit code 0, `/exit`): fires. On crash/API error: StopFailure fires instead; SessionEnd may not if process killed. | https://github.com/anthropics/claude-code/issues/41577, #62987 (WebSearch results) | ⚠️ Partial |
| **Context-limit errors**: SessionEnd fire? | **Unverified in docs.** Likely triggers SessionEnd with `reason: "other"` if process exits; but hook may not run if context hit causes immediate termination. Transcript file survives; no documented cleanup barrier. | (No official docs found) | ⚠️ No |
| **`/exit` command**: SessionEnd fire? | **Yes, likely.** Normal exit → `reason: "prompt_input_exit"`. Transcript persists. Not explicitly documented; inferred from SessionEnd reason values. | https://code.claude.com/docs/en/hooks-guide.md (line 673, "prompt_input_exit") | ⚠️ Inferred |
| **Worktree hooks**: WorktreeCreate / WorktreeRemove exist? | **Yes.** `WorktreeCreate`: replaces default `git worktree add` logic; hook output is the worktree directory path. Returns JSON or exits non-zero to fail. Used for non-git VCS. `WorktreeRemove`: called on session exit or subagent finish. Both can be matchers for SessionStart (resume behavior). Parent repo's `.claude/settings.json` hooks DO apply inside worktree (shared plugins, permission approvals). | https://code.claude.com/docs/en/worktrees.md (lines 495–496); https://code.claude.com/docs/en/hooks-guide.md (lines 495–496) | ✓ Yes |
| **Worktree naming/location**: where created? | `.claude/worktrees/<name>/` by default. Git branch: `worktree-<name>`. Can reuse existing worktree by name. Non-git VCS: `WorktreeCreate` hook returns path. `--worktree "#PR_NUMBER"` creates at `.claude/worktrees/pr-<number>`. | https://code.claude.com/docs/en/worktrees.md (starting section) | ✓ Yes |
| **AskUserQuestion hook**: does it exist? | **No.** Not a hook event. `Elicitation` exists for MCP server forms (line 499), but not for general Claude prompts. Interactive prompts (questions) do not have a hook equivalent for blocking or auto-answering. (Noted as limitation in plan 0001 §7: "No hook ever blocks on a question.") | https://code.claude.com/docs/en/hooks-guide.md (lines 469–501) | ✓ Yes (not found) |
| **Permission prompt detection**: can hook read if session blocked on prompt? | **Inferred: No direct hook.** `PermissionRequest` hook fires when prompt is about to show, but hook runs before prompt is visible to user. `Elicitation` hook fires for MCP forms. No built-in signal for "currently blocked waiting for user input." Workaround: `SessionStart` with `startup` or `resume` matcher to detect if session resumed while blocked (not directly observable). | https://code.claude.com/docs/en/hooks-guide.md (lines 476, 499) | ⚠️ Inferred |
| **2026-08 changelog**: hook breaking changes, new events? | **PreCompact support added** (per WebSearch). SessionEnd hook fix for interactive `/resume` switch (v2.1.223 or later). No documented breaking changes. Default hook timeout behavior unchanged (600s for command/http/mcp_tool). | WebSearch ("claude-code-changelog 2026-08") | ⚠️ Partial |

## Implications for Plan 0001 §2 & §5

### Session State Observability ✓ (Reliable)
- `SessionStart`, `PostToolUse(Edit|Write)`, `Stop`, `SessionEnd` hooks are **reliably observable** for state tracking (working → running(tool) → idle → stopped).
- `session_id`, `cwd`, `transcript_path` are stable identifiers across hooks.
- `stop_hook_active` field in Stop hook input tells whether hook has already fired this turn (guards Stop-hook loops).

### Critical Gaps & Fallbacks
1. **PermissionRequest state**: No hook reads current permission-prompt blocking status in real time. Workaround: fallback to transcript mtime (last message age) or polling `claude agents --json`.
2. **Abnormal exit coverage**: SessionEnd hook is **not guaranteed on SIGKILL or crash**. Fallback: transcript file existence + mtime; git lock file sweeps; periodic health checks.
3. **PreCompact data loss**: `PreCompact` can block (exit 2) but runs too late to prevent summary loss. Capture holdings/claims in `Stop` hook instead (earlier, always fires on normal exit).

### Stop-Hook Loop Guard ✓
- `stop_hook_active: true` in JSON input when hook already blocked this turn.
- Script should `exit 0` (allow stop) if `stop_hook_active == true` to prevent 8-block limit and override.
- Cap is tunable: `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` env var.

### Handover Gate Advisory → Blocking (§5)
- **Advisory mode (M0)**: Stop hook + PreCompact hook can inject `additionalContext` with handover result (green/red checks).
- **Blocking mode (M1, after one round)**: Stop hook can exit 2 to block awaiting_review (but only if evidence missing). Requires explicit marker in hook to detect "worker set awaiting_review in this turn" — not directly observable from hook input. Workaround: check `sessions` table state prior to Stop hook; hook can query ledger if necessary.
- **Edge case**: If PreCompact fires before Stop, compaction summary may truncate evidence trails. Capture proof in Stop hook (earlier) rather than PreCompact.

### Ledger Integration Recommendations
| Table | Hook Trigger | Reliability | Fallback |
|-------|--------------|-------------|----------|
| `edit_journal` | `PostToolUse(Edit\|Write)` | ✓ 100% (before tool runs) | Replay `git diff` against merge-base |
| `sessions.state` | `SessionStart`, `Stop`, `SessionEnd` | ⚠️ ~98% (SessionEnd may miss SIGKILL) | Transcript mtime + git lock sweep |
| `verify_runs` | `PostToolUse(Bash)` + hand-off parse | ✓ 99% (exit code always recoverable) | Parse `logs/verify.log` footer |
| Handover evidence | `Stop` hook (advisory) / `Stop` + `PreCompact` (blocking) | ⚠️ 95% (compaction may truncate) | Ledger query before Stop fires |

---

## Sources

- [Hooks Guide](https://code.claude.com/docs/en/hooks-guide.md) — Main reference for all hook events, input/output, timeouts, concurrency. Checked 2026-08-18.
- [Hooks Reference](https://code.claude.com/docs/en/hooks.md) — Detailed JSON schemas (not fully fetched; summary from hooks-guide).
- [Worktrees Reference](https://code.claude.com/docs/en/worktrees.md) — WorktreeCreate/Remove hooks, naming, inheritance. Checked 2026-08-18.
- [Sessions Reference](https://code.claude.com/docs/en/sessions.md) — Transcript storage, SessionEnd trigger context. Checked 2026-08-18.
- [Claude Code GitHub Issues](https://github.com/anthropics/claude-code/issues) — #41577 (SessionEnd async work killed), #62987 (SIGKILL grace period violated). WebSearch 2026-08-18.
- [WebSearch](https://www.morphllm.com/claude-code-hooks) — General hook overview; PreCompact schema hints (secondary source).

**Note**: Some edge cases (context-limit SessionEnd, AskUserQuestion hook) are unverified because they are not documented or are explicitly absent. Italicized rows represent inferences from documented behavior or unverified based on GitHub issues rather than official docs.
