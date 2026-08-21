# CLAUDE.md — Air (`ai_runner`)

Rules that apply without being looked up, and indexes of where everything else lives.
Index items are 1–3 lines; detail lives behind the link.

## Rules

- **No Claude memory for this project. Ever.** It is an opaque, untrackable surface. Rules and
  decisions live here and in `docs/`. If memory files exist for this project, delete them and
  move the content into `docs/decisions.md`.
- **Source trail always.** Every research claim cites a primary source — URL with access date,
  or `path:line-range`. Claims derived from adopter notes cite the note *and* the source it
  cited. No "mysterious bunch of claims".
- **Do less. Nothing is sacred.** The goal is the model's productivity, not the framework's
  completeness. Air removes friction (relayed facts, drift, collisions); it does not direct the
  work. **Invoke the `do-less` skill before any change or addition**, and when reviewing what
  exists; it is the most important skill in this repo. Every constraint, hook, deny rule, role
  line, or procedure must name the recorded failure it prevents AND the condition under which
  it is removed; guardrails that are never re-examined become the throttles a more capable model
  does not need. When a rule can be a measurement, measure; when in doubt, leave it out. Nothing
  is off the table, including the multi-agent pattern itself: if one capable session does the
  work better than a coordinator plus workers, the fleet goes. Gas Town is the opposite and the
  warning. Evidence: `docs/research/guardrails-as-throttles.md` (owner, 2026-08-21).
- **Only build what makes sense.** Nothing is built without a named pain from the record it
  removes, and it ships with a red/green probe that proves it fires. Gas Town is the cautionary
  case (`docs/research/beads-and-gastown.md §2.5`).
- **A human is always in the loop.** Core requirement, not a phase. Every agent session is a
  terminal the owner can watch and type into (today: one coordinator + three workers); Air's
  launchers start interactive sessions, never headless ones, and nothing Air builds may take the
  owner out of the loop or hide what an agent is doing. Introspection into live state
  (`air status`, the event stream) is part of the same requirement.
- **Productive sooner than later.** Improve adopter's current process incrementally; every early
  milestone is something adopter can run. Prefer replacing one prose rule with one enforced
  check over designing a platform.
- **Machinery over Markdown.** Every rule that can be a deny rule, a hook check, a launcher flag,
  a tool schema, or an injected fact should be one; Markdown is for the *why* and for judgement
  that cannot be encoded. When a prose rule becomes machinery, delete the prose (owner,
  2026-08-20).
- **Rust.** Prefer using or borrowing from an existing good project; research must show why not
  before we build. Never make a target repo's tooling depend on Air's *build* — install a binary.
- **Steal avidly** from `~/projects/adopter` and `~/projects/metis` (and cite what was taken).
- **Tests are optimized for speed, always.** They run constantly; per-test cost is a first-class
  constraint (in-memory SQLite, temp git repos, no sleeps, no network, parallel-safe).
- **This file** is rules + indexes + essentials only. Plans, framing, and decisions go in `docs/`.

## Index — documents

| Read when | Document |
|---|---|
| Wanting the "why", framing, and every owner decision (dated) | [`docs/decisions.md`](docs/decisions.md) |
| Orienting in the research | [`docs/README.md`](docs/README.md) — index of all reports |
| Deciding what shape Air is and why | [`docs/research/SYNTHESIS.md`](docs/research/SYNTHESIS.md) |
| Building on the first slice (ledger facts, hooks, the one refusal, evidence weighting) | [`docs/plans/0001-first-slice.md`](docs/plans/0001-first-slice.md) |
| The first-round surface as built (claims wrap bd, capture/triage, status/attention, `air mcp` channel, install, launchers) and how to operate it on adopter | [`docs/plans/0004-first-round-surface.md`](docs/plans/0004-first-round-surface.md) |
| Integrating Air into a target repo (install, rules to change, self-maintenance) | [`docs/rules/adopting-air.md`](docs/rules/adopting-air.md) |
| What comes next, in order | [`docs/plans/0005-roadmap.md`](docs/plans/0005-roadmap.md) |
| Which role an agent is and what it may do | [`docs/rules/roles.md`](docs/rules/roles.md) · research: [`docs/research/agent-roles-and-confinement.md`](docs/research/agent-roles-and-confinement.md) |
| Decomposing a feature, sizing beads, cutting per-worker queues | skills `decomposition`, `phase-transitions`; research: [`docs/research/metis-decomposition-and-agile.md`](docs/research/metis-decomposition-and-agile.md) |
| Which metrics Air records (the single list) | [`docs/research/verification/ticks/2026-08-18-0430-measurement-spec.md`](docs/research/verification/ticks/2026-08-18-0430-measurement-spec.md) |
| Running the work procedure: capture → triage → bead, decomposition, dispatch, hand-over, landing (the single procedure; what Air enforces vs judgement) | [`docs/plans/0002-what-to-work-on.md`](docs/plans/0002-what-to-work-on.md) |
| Porting or writing a skill | a private skills inventory; ported skills live in `.claude/skills/` with a `## Provenance` footer each and an index in [`.claude/skills/PROVENANCE.md`](.claude/skills/PROVENANCE.md) |
| Writing prose, docs, commits, PRs, tests, reviews | Use the skills: `writing-style`, `writing-docs`, `commits`, `writing-pr-descriptions`, `writing-rust-tests`, `review`, `rust-safety`, `beads`, `parallel-worktrees` — see `.claude/skills/` |
| Rust conventions (errors, lints, MSRV — open decisions) | [`docs/plans/0003-rust-conventions.md`](docs/plans/0003-rust-conventions.md) |
| Worktree protocol for this repo | [`docs/rules/worktree-protocol.md`](docs/rules/worktree-protocol.md) · [`docs/rules/writing.md`](docs/rules/writing.md) |
| Touching billing/cost assumptions | [`docs/research/claude-code-billing.md`](docs/research/claude-code-billing.md) — primary sources only |
| Working with `bd` (versions, leases trap) | [`docs/research/beads-and-gastown.md`](docs/research/beads-and-gastown.md) §0 |

## Index — systems and subsystems (first slice 2026-08-18, plan 0001; first-round surface 2026-08-20, plan 0004)

| System | One line |
|---|---|
| `crates/ledger` (`air-ledger`) | SQLite WAL ledger at the main checkout (`.air/ledger.db`) + NDJSON events (`.air/events/`): `verify_runs`, `edit_journal`, `claims`, `sessions` (with `role`), `landings`, `captures` (schema v2). No time-based expiry. **Built.** |
| `crates/hooks` (`air-hooks`) + `air hook` | Hook I/O types, the pure hand-over gate, edit journal; `air hook` dispatches SessionStart/PreToolUse/PostToolUse/PermissionRequest/Stop/SessionEnd, fails open, ~100 ms, **one event line per invocation** (transitions included). Advisory unless `AIR_ENFORCE=1`. **Built.** |
| `crates/cli` (`air`) | Built: `record` (command/duration/bytes/dirty, suspicious flags), `handover` (4 checks incl. digest), `holdings`, `claim`/`release` (wrap `bd --claim`), `capture [--for owner]`/`inbox [--owner]`/`triage`, `lease take|release|status|break|beat`, `status [--attention]`, `mcp`, `install`, `worker`/`coordinator` (+ `.claude/air.json` deny patterns), `hook`, `doctor`, `selftest` (12 probes). Next (after round-one data): `next`, `peer`, `merge-advice`, `land`, `gc`, PreCompact re-inject. |
| `air mcp` | One stdio MCP server: the coordinator's **channel** (pushes attention conditions from a ledger poll; no sockets, no timers) plus tools (`air_*`) and resources (`air://status`, …) that invoke the CLI. Synchronous, bounded, panic-isolated. **Built.** |
| Launchers `air worker <name>` / `air coordinator` | Interactive `claude` with native worktree isolation, roles prose appended, deny list that holds in every permission mode, env instead of drifting files; coordinator gets the channel. **Built.** |
| Hand-over gate | The one refusal: `awaiting_review`/close needs recorded green at HEAD + main merged. Never blocks a prompt or a WIP commit. |
| `crates/bd` (`air-bd`) | `WorkLedger` trait + `bd --json` shell-out (bd 1.2.2 surface); CAS/leases live in the ledger; never called from a hook. **Built (minimal).** |
| Coordinator (human-facing session) | Steers, triages the capture inbox, builds per-worker queues in beads fields, rulings, arbitration, `land`. Informed by the Air channel, not woken by cron. SendMessage stays the agent-to-agent channel. |

## Essentials

- Live adopter fleet is running on this machine (`~/projects/adopter`); never modify its state
  from here.
- Owner is `29932896+AJTJ@users.noreply.github.com`; commits are authored `ajtj`.
