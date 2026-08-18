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

## Layout

| Path | What |
|---|---|
| `docs/research/` | Sourced research reports — every claim traces to a file:line or URL |
| `docs/plans/` | Design arguments and decisions (ADR-style) |
| `docs/README.md` | Index and read-when triggers |

## Rules

- **Source trail always.** Any research claim cites its primary source (URL with access date, or
  `path:line`). Notes derived from adopter cite the adopter file *and* the original source it
  cited.
- Rust. `cargo` workspace once design is settled.
