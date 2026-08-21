# Air

A hub and referee for a few Claude Code agents working in git worktrees on one repository,
coordinated by [beads](https://github.com/steveyegge/beads). One Rust binary, `air`. It keeps
the loop you already run (claim, worktree, work, verify, review, land) and removes the parts that
cost turns: facts relayed through chat, "green" that drifts from the commit it was measured on,
and not knowing who is in which file.

Air does not run agents, does not replace beads, and does not take the human out of the loop.
Every agent session is a terminal you can watch and type into.

## What Air offers

**Records** (in `<repo>/.air/`, shared by every worktree, gitignored)

- `air record verify -- <cmd>`: "commit X passed check Y at time T". The fact the hand-over gate
  checks; never model text.
- An edit journal filled by hooks at zero token cost: who is touching which file.
- `air claim <bead>` / `air release <bead> --reason <r>`: the claim history beads does not keep
  (when, declared files, hand-over attempts, why it was given back). Wraps `bd update --claim`;
  beads stays the atomic source of truth.
- `air capture "<text>"`: one line into an inbox that is not `ready`. Workers capture; they
  never file beads.
- An append-only event log (`.air/events/YYYY-MM-DD.ndjson`): every question Air was asked,
  its answer, its reason, and what it looked at. Nothing is overwritten.

**Answers**

- `air status`: every worker's session state, HEAD, green-at-HEAD, open claims with hand-over
  attempts, files held, file overlaps between workers, inbox depth.
- `air status --attention`: only what needs a human or the coordinator right now: a worker
  stuck on a permission prompt, idle or silent while holding a claim, gone with a claim, handing
  over without green, captures waiting. Thresholds via `AIR_ATTENTION_*_MIN`.
- `air holdings [--file X]`: who has edits in which files across worktrees.
- `air handover`: is this worktree ready to hand over, and if not, exactly which command fixes it.

**Refuses one thing**

- Setting a bead to `awaiting_review` or closing it without a recorded green verify at HEAD
  that contains current `main`. Advisory for the first round (prints what it would refuse);
  `AIR_ENFORCE=1` makes it real. It never blocks a prompt, a WIP commit, or a merge.

**Informs the coordinator instead of waking it**

- `air mcp` is one MCP server that is both a Claude Code *channel* and a tool surface. It
  re-evaluates the attention conditions from the ledger every 30 s and pushes new or escalated
  ones into the coordinator's session. No cron, no polling by the agent. The same surface is
  available as tools (`air_status`, `air_claim`, `air_capture`, …) and resources
  (`@air://status`, `@air://inbox`).

**Launches sessions with their role applied**

- `air worker <name>`: `claude --worktree <name>` with the roles document appended to the
  system prompt, a deny list that holds in every permission mode (`air land`, `git push`,
  `bd create`, `bd sync`, raw `bd update --claim`, nested `claude`, leaving the worktree), and
  `AIR_ROLE` / `BEADS_ACTOR` set by flag instead of by files that drift.
- `air coordinator`: `claude` in the main checkout with the Air channel attached and commits
  and pushes denied (the coordinator steers; it does not do worker work on main).

## What a target repository needs

Verified 2026-08-20 against adopter (the first target) and Claude Code 2.1.238.

| Needs | Why |
|---|---|
| A git repository using linked worktrees (`git worktree add`, or `claude --worktree`) | Role is the checkout: main is the coordinator, each worktree a worker. The ledger lives at the main checkout and is found via `git rev-parse --git-common-dir`. |
| [beads](https://github.com/steveyegge/beads) initialised (`.beads/`), bd 1.2.x on PATH | Air wraps `bd update --claim`, `bd update -s open`, and reads `bd --json`. Pin 1.2.2. |
| A verify command that exits non-zero on red (`make verify`, `cargo test`, …) | `air record verify -- <cmd>` records its exit against HEAD. The gate needs this fact. |
| Claude Code ≥ 2.1.211 | Per-worktree `settings.local.json` moved to the main checkout in 2.1.211, which is why Air sets role and env by launch flag. `--append-system-prompt-file`, `--disallowed-tools`, `--settings`, `--worktree`, and the channel flags parse on 2.1.238. |
| `.air/` in `.gitignore` | The ledger and event log are local state. `air install` advises if missing. |

Nothing in the target repo's build or tooling depends on this repository. Install the binary;
the repo only ever sees `air` on PATH.

## Onboarding a repository

```bash
# 1. Install the binary so the `air` on PATH is this build.
cargo install --path crates/cli
air selftest                      # 10 red/green probes; every check proves it fires

# 2. In the target repo's main checkout: see what install would change, then apply it.
cd ~/projects/<repo>
air install                       # dry run: prints the merged .claude/settings.json and .mcp.json
air install --write               # refuses if `air` on PATH is not this binary

# 3. Record verify runs. Either change the habit or the Makefile target:
air record verify -- make verify

# 4. Start the sessions, one per terminal.
air coordinator                   # main checkout; channel attached
air worker frontend               # creates or reuses the worktree; interactive
air worker backend
```

What `air install --write` touches, and only that:

- `.claude/settings.json`: adds `air hook` entries for `SessionStart`, `PreToolUse`,
  `PostToolUse`, `PermissionRequest`, `Stop`, `SubagentStop`, `SessionEnd` (5 s timeout).
  Existing entries are preserved; re-running changes nothing.
- `.mcp.json`: adds the `air` server (`air mcp`). Existing servers are preserved.
- `.air/roles.md`: the roles document the launchers append to the system prompt.
- `.claude/skills/air-decomposition/`, `.claude/skills/air-phase-transitions/`: the
  coordinator's procedures, loaded on demand.

Beads side: always create beads with `bd create --validate --estimate <minutes>`. bd refuses a
task, feature, or bug whose description lacks an `## Acceptance Criteria` heading (compiled
in; `bd lint --help`). The coordinator then links the capture: `air triage <id> --bead <new>`.

For the full integration package (rules to change in the repo, what Air replaces, how the
integration stays current) see `docs/rules/adopting-air.md`.

## Day to day, by role

**Worker** (in a worktree): `bd ready` → `air claim <bead> --files a,b` → work, commit small →
`git merge main` → `air record verify -- <cmd>` → `air handover` → `bd update <bead> -s
awaiting_review`. Anything discovered outside the bead: `air capture "<one line>"`. Giving up:
`air release <bead> --reason abandoned|false-premise|…`.

**Coordinator** (main checkout): reads `air status`; gets attention conditions pushed by the
channel; triages with `air inbox` then `bd create --validate --estimate N` and `air triage <id>
--bead <new>` (or `--drop "<why>"`); builds each worker's queue with beads fields only
(`assignee`, priority, `blocks` edges); lands (landing command is the repo's own until `air
land` is built).

**You**: any terminal, `air status --attention`, `air holdings`, `jq` over
`.air/events/*.ndjson`.

## What Air never does

Sends messages between agents (that stays `SendMessage`), writes to beads except through the
`bd` commands above, runs anything headless, pushes, decides what to work on, or expires a
record on a timer. Everything it refuses names the rule and the fixing command.

## Repository map

`crates/ledger` (SQLite + events), `crates/hooks` (hook I/O, the pure gate, journal),
`crates/bd` (the beads boundary), `crates/cli` (`air`). `docs/decisions.md` holds every owner
decision, dated; `docs/plans/0001` and `0004` say what was built and why; `docs/rules/roles.md`
is what agents read; `docs/research/` is the evidence, every claim with a source.

## Uninstall

Remove the `air hook` entries from `.claude/settings.json` and the `air` server from
`.mcp.json`; delete `.air/`. Beads state is untouched.
