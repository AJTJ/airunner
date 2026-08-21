# 0004 — First-round surface: claims, capture, status, install, launchers, `air mcp`

Status: decided and built, 2026-08-20 (owner decisions in [`../decisions.md`](../decisions.md)
under that date). Extends [`0001-first-slice.md`](0001-first-slice.md); supersedes its §4 check 4
("CAS owned by the ledger") and its §9 non-goal on MCP.

## 1. What the first adopter round needs, and why each piece exists

| Piece | Named pain it removes | Probe |
|---|---|---|
| One event line per hook invocation, session transitions included | First round is for information; in-place rows lose the sequence | `hook.rs` test: seven hooks, seven lines |
| `air claim` / `air release` (wrap `bd update --claim`) | Claim history bd does not keep (attempts, release reason); no watchers ("a missed event must not be possible") | fake-bd test: order and nothing-on-refusal; selftest `claim` |
| `air capture` / `inbox` / `triage` | Workers must not file beads; the inbox is not `ready` | CLI round-trip test |
| `air status [--attention]` | The coordinator re-derives fleet state from chat; needs one screen and a deterministic "needs a human" list | pure `attention()` tests; selftest `attention` |
| `air mcp` = channel + tools + resources | The coordinator was woken by cron; it should be informed when a condition holds. Channels are the only documented push into a session | stdio test: handshake, garbage line, push, EOF exit, RSS canary |
| `air install` | Hook wiring by hand drifts; the binary the hooks resolve to must be this one | merge idempotence test; PATH refusal test |
| `air worker` / `air coordinator` | Per-worktree settings files drifted (`BEADS_ACTOR` wrong in two worktrees); deny rules must hold in every permission mode; sessions must stay interactive | argv tests; `--print` test |

## 2. Shape

- **bd is truth; Air wraps it.** `air claim` runs `bd update --claim` (bd's atomic CAS decides
  races) and writes the ledger row only after bd succeeded. Raw `bd update --claim` and
  `bd create` are denied to workers at launch.
- **Role is the checkout.** `main` is the coordinator; a worktree is a worker. Hooks record it;
  launchers set `AIR_ROLE` as a label.
- **Informed, not woken.** `air mcp` evaluates the attention conditions from the ledger every
  `AIR_CHANNEL_POLL_SECS` (30) and pushes new or escalated ones through
  `notifications/claude/channel`. No sockets, no timers in the coordinator, no daemon beyond the
  server Claude Code already keeps alive for the session.
- **One implementation.** MCP tools and resources invoke the `air` CLI with `--json`; they cannot
  disagree with the command line.
- **Human in the loop.** Launchers `exec` an interactive `claude`; nothing headless.

## 3. Long-running guarantees (`air mcp`)

Synchronous, no async runtime. Bounded line reader (4 MB). Locked writer that recovers from
poisoning. Every child process time-bounded, drained on threads (no 64 KB pipe deadlock), killed
and reaped on timeout. Poll tick and request handler each wrapped in `catch_unwind`. De-dupe map
bounded by workers × kinds and pruned when a condition clears. Clean exit on stdin EOF. Test
canary: RSS flat across 3000 requests.

## 4. Not built (named triggers)

`next`, `peer`, `merge-advice`, `land`, `gc`: after one round of ledger data says which relay
still costs turns. `--agent` launcher form, Bash sandbox, headless workers: see
[`../research/agent-roles-and-confinement.md`](../research/agent-roles-and-confinement.md) §6.
Role-based deny inside `air hook` for shapes the pattern matcher misses: when the launcher's
deny list is seen to be bypassed.

## 5. Operating it (adopter side; the owner does these, never this repo)

1. `cargo install --path crates/cli` so `air` on PATH is this binary.
2. In the adopter main checkout: `air install` (read the plan), then `air install --write`.
3. Always `bd create --validate --estimate <min>`: bd already refuses a task/feature/bug whose
   description lacks `## Acceptance Criteria` (compiled in per type, `bd lint --help`; it is a
   heading grep, not a content check). No config change needed. Pin bd at 1.2.2 (this machine
   has 1.2.1 on PATH as of 2026-08-20).
4. `make verify` target (or the habit) becomes `air record verify -- <cmd>`.
5. Coordinator terminal: `air coordinator`. Worker terminals: `air worker <name>`.
6. After the round: `jq` over `.air/events/*.ndjson`; `air status --json`.

Verified 2026-08-20 on Claude Code 2.1.238: `--append-system-prompt-file`, `--disallowed-tools`,
`--settings`, `--worktree` are listed in `--help`; `--channels` and
`--dangerously-load-development-channels` are accepted by the parser (not listed; research
preview). adopter's `.claude/settings.json` today has only `bd prime` on `SessionStart` and
no `.mcp.json`, so `air install` adds rather than conflicts.
