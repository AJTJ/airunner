# ai_runner

An AI orchestration runtime, in Rust.

**Problem.** `projects/adopter` runs a fleet of Claude Code agents in git worktrees, coordinated
by [beads](https://github.com/steveyegge/beads), landed through a `make land` gate. It works
somewhat well — but the *process* (claim → worktree → work → verify → review → land → close) lives
as prose in `CLAUDE.md` and `docs/rules/`, which is not binding. Steps get skipped; checks drift.

**Goal.** Codify that process into steps and checks that are enforced by machinery. Keep beads as
the coordination layer. Steal avidly from adopter (its research, its skills, its retrospectives)
and from [metis](https://github.com/colliery-io/metis); reuse existing prior art wherever a good
project already exists.

**Open question (honest).** It is not yet established that a full "runtime" is what is needed, as
opposed to a thinner enforcement layer (hooks + gates) on top of beads and Claude Code. The
research in `docs/research/` is meant to settle that before code is written.

## What Air does (first slice, as designed — see `docs/plans/0001-first-slice.md`)

Air is a hub and a referee for a few concurrent coding agents working in git worktrees on one
repo, coordinated by beads. It keeps the loop you already have and removes the parts that cost
turns: relayed facts, "green" that drifts, and not knowing who is in which file.

**Records** — `record verify` (commit X passed at T), an edit journal (who is touching which
file), `claim`/`release` (the intent record; CAS lives here), `capture` (one line into an inbox
that is not ready — workers never decide placement), session state (working / running / stuck /
idle), landing receipts, an events log with reasons and denominators.

**Answers** — `holdings`, `peer <name>` (green sha, red or green, the exact merge to run),
`merge-advice`, `next` (live, overlap-ranked, shows why), `status` (the coordinator's one
screen), `post-merge` (citation/fitness check + the fix).

**Refuses one thing** — `handover`: awaiting_review/close needs recorded green at HEAD and main
merged; prints what is missing and the command. Advisory for the first round. Never blocks a
prompt, a WIP commit, or a merge.

**Lands** — `land <worker> [--sha X]`: adopter's `land.sh` behaviour-for-behaviour, plus
land-by-sha, generated-input regeneration, and a receipt.

**Keeps itself honest** — `selftest` (every check has a red/green probe), `doctor`, `gc`
(state-based, no timers), `install` (verifies the resolved hook path).

**Measures for free** — review latency (wait vs rewind), first-hand-over success, discarded
hours, imported-red incidents, stale-`next` rate, WIP as a counter.

**Deliberately does not** — send messages, spawn or supervise sessions (a later layer), choose
features, run an LLM coordinator, keep phase labels, batch/bisect merges, resolve conflicts, or
store anything git or `bd` can re-derive.

## Quick start (2026-08-20)

```bash
cargo install --path crates/cli          # `air` on PATH must be this binary
cd ~/projects/<target-repo>
air install                              # dry run: shows the hook + .mcp.json merge
air install --write
air coordinator                          # main checkout, channel attached
air worker <name>                        # one per worktree; interactive
air status --attention                   # what needs a human right now
air selftest                             # 10 red/green probes
```

See `docs/plans/0004-first-round-surface.md` for what each command is for and the adopter-side
steps (beads template, bd pin, `air record verify`).

## Layout

| Path | What |
|---|---|
| `docs/research/` | Sourced research reports — every claim traces to a file:line or URL |
| `docs/plans/` | Design arguments and decisions (ADR-style) |
| `docs/README.md` | Index and read-when triggers |
| `crates/` | `ledger`, `bd`, `hooks`, `cli` (binary `air`) |
| `.claude/skills/` | Ported/written skills with provenance |

## Rules

- **Source trail always.** Any research claim cites its primary source (URL with access date, or
  `path:line`). Notes derived from adopter cite the adopter file *and* the original source it
  cited.
- Rust. `cargo` workspace once design is settled.
