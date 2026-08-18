# Research-deepening ticks — ledger

Owner-scheduled: every 15 minutes from 02:24 to 05:24 PDT on 2026-08-18 (13 ticks). Each tick
takes ONE gap or weak claim, researches it against primary sources, writes one file here, and
appends one line below. SUMMARY.md is written on the last tick.

| Tick file | Topic | Verdict (one line) |
|---|---|---|
| (02:15 tick — Step 0) | Relaunch after session-limit stall (01:50 reset) | Nothing survived the 23:xx launch. Relaunched: skill-port groups 1–5; verification slices fleet-size + specs/tooling. Pending slices: mas-literature part 1, part 2, protocols-leases-resources (launch on later ticks as capacity frees). |
| (02:30 tick) | Step 0: skills 5/5 done; specs slice done; launched MAS part 1. Step 1: what-to-work-on process → agent writing `2026-08-18-0230-what-to-work-on.md` + draft plan 0002 | pending |
| `2026-08-18-0230-what-to-work-on.md` | Feature → epics → beads and epic traversal (Metis, beads, Gas Town, Symphony, spec-kit, 0022, Anthropic/Codex) | Converges on: epic + `blocks` edges + `bd ready` frontier is enough; the one thing to enforce is the triage commitment point (executable acceptance + lane + edges + citations) plus non-empty `--design`; done = children closed + epic check green on main. Do NOT build formulas/molecules, convoys, integration branches, a phase machine, or spec-kit's triple. Plan 0002 drafted. |
| `2026-08-18-0245-claude-code-hook-edge-cases.md` | Hook events (PermissionRequest, PreCompact, SessionEnd, Stop, SubagentStop), timeouts, concurrency, abnormal exits, worktree hooks, gap on AskUserQuestion | Reliably observable: SessionStart/Stop/SessionEnd/PostToolUse(Edit\|Write). Critical gap: SessionEnd NOT guaranteed on SIGKILL/crash (fallback: transcript mtime + lock sweep). PreCompact too late for evidence capture; use Stop hook instead. Handover gate can be advisory (M0) via Stop+PreCompact `additionalContext`, then blocking (M1) via Stop hook exit 2 + pre-flight ledger query. Stop-hook loops guarded by `stop_hook_active` field + 8-block cap (tunable). No hook for permission-prompt state or user questions (by design). |
