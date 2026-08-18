# CLAUDE.md — ai_runner

Rules that apply without being looked up, and an index of where everything else lives.
Detail lives behind links in [`docs/README.md`](docs/README.md).

## Hard rules

- **No Claude memory for this project. Ever.** The `~/.claude/projects/…/memory/` directory is an
  opaque surface that cannot be tracked. Anything worth remembering goes in this file (rules,
  constraints, owner decisions) or in `docs/` (research, plans). If you find memory files for this
  project, delete them and move the content here.
- **Source trail always.** Every research claim cites its primary source — a URL with access date,
  or `path:line-range`. A claim derived from an adopter note cites the adopter file *and* the
  original source that note cited. No "mysterious bunch of claims".
- **Rust.** Prefer using or borrowing heavily from an existing good project over building; the
  research must show why not before we build.
- **Productive sooner than later.** Learn from and improve adopter's *current* process
  incrementally; do not start from scratch or wander far afield. Every early milestone must be
  something adopter can actually run. Prefer replacing one prose rule with one enforced check
  over designing a platform.
- **Steal avidly** from `~/projects/adopter` (research, skills, retrospectives) and
  `~/projects/metis`. Cite what was taken.

## What this project is (owner's framing, 2026-08-17 — treat as hypotheses, not decisions)

- Upgrade the multi-agent fleet system in `~/projects/adopter` (Claude Code agents in git
  worktrees, coordinated by `bd`/beads, landed via `make land`).
- Diagnosis: the process there (claim → worktree → work → verify → review → land → close) is prose
  in `CLAUDE.md`/`docs/rules` — not binding. Goal: codify it into steps and checks enforced by
  machinery, i.e. a codified coordination layer rather than docs.
- **Beads works already** and should stay as the coordination layer.
- Topology: today one coordinator + many workers (+ possibly other services later). Topology should
  be **flexible and named** — declared, pluggable topologies as a first-class feature.
- Cost: workers should be Claude Code sessions in worktrees (subscription billing); API-key/SDK
  usage is a different pricing model that could "start costing tons of money". Open to a better
  way if it is cheap. See `docs/research/claude-code-billing.md`.
- Worker backends should be pluggable: a role may be backed by a Claude Code session (subscription)
  OR a direct AI API call (any provider), with cost reported in a common unit. Billing info must be
  current and from Anthropic primary sources.
- **Open questions the owner has NOT answered — do not assume:**
  1. Is a full "runtime" needed, or a thinner enforcement layer on top of beads + Claude Code?
  2. Should the runtime *drive* Claude Code (spawn/supervise), or should Claude Code *call into*
     the runtime (MCP/CLI, the way beads and metis work)? Or both?

## Index

See [`docs/README.md`](docs/README.md) — research reports under `docs/research/`, the synthesis at
`docs/research/SYNTHESIS.md`, design arguments under `docs/plans/`.
