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
- **Only build what makes sense.** Nothing is built without a named pain from the record it
  removes, and it ships with a red/green probe that proves it fires. Gas Town is the cautionary
  case (`docs/research/beads-and-gastown.md §2.5`).
- **Productive sooner than later.** Improve adopter's current process incrementally; every early
  milestone is something adopter can run. Prefer replacing one prose rule with one enforced
  check over designing a platform.
- **Rust.** Prefer using or borrowing from an existing good project; research must show why not
  before we build. Never make a target repo's tooling depend on Air's *build* — install a binary.
- **Steal avidly** from `~/projects/adopter` and `~/projects/metis` (and cite what was taken).
- **This file** is rules + indexes + essentials only. Plans, framing, and decisions go in `docs/`.

## Index — documents

| Read when | Document |
|---|---|
| Wanting the "why", framing, and every owner decision (dated) | [`docs/decisions.md`](docs/decisions.md) |
| Orienting in the research | [`docs/README.md`](docs/README.md) — index of all reports |
| Deciding what shape Air is and why | [`docs/research/SYNTHESIS.md`](docs/research/SYNTHESIS.md) |
| Building the first slice (ledger facts, hooks, the one refusal, evidence weighting) | [`docs/plans/0001-first-slice.md`](docs/plans/0001-first-slice.md) |
| Thinking about feature → epics → tasks and how agents traverse an epic | `docs/plans/0002-what-to-work-on.md` (to be written) |
| Porting or writing a skill | a private skills inventory; ported skills live in `.claude/skills/` with a `## Provenance` footer each and an index in [`.claude/skills/PROVENANCE.md`](.claude/skills/PROVENANCE.md) |
| Writing prose, docs, commits, PRs, tests, reviews | Use the skills: `writing-style`, `writing-docs`, `commits`, `writing-pr-descriptions`, `writing-rust-tests`, `review`, `rust-safety`, `beads`, `parallel-worktrees` — see `.claude/skills/` |
| Rust conventions (errors, lints, MSRV — open decisions) | [`docs/plans/0003-rust-conventions.md`](docs/plans/0003-rust-conventions.md) |
| Worktree protocol for this repo | [`docs/rules/worktree-protocol.md`](docs/rules/worktree-protocol.md) · [`docs/rules/writing.md`](docs/rules/writing.md) |
| Touching billing/cost assumptions | [`docs/research/claude-code-billing.md`](docs/research/claude-code-billing.md) — primary sources only |
| Working with `bd` (versions, leases trap) | [`docs/research/beads-and-gastown.md`](docs/research/beads-and-gastown.md) §0 |

## Index — systems and subsystems (planned; see plan 0001)

| System | One line |
|---|---|
| Ledger (`.air/ledger.db` + `.air/events/`) | SQLite of facts git/bd can't re-derive: `verify_runs`, `edit_journal`, `claims`, `sessions`, `landings`; NDJSON events. No time-based expiry. |
| `air hook` | One binary behind Claude Code hooks (SessionStart / PreToolUse / PostToolUse / PermissionRequest / PreCompact / Stop); answers in <300 ms; advisory first. |
| `air` CLI | `record`, `claim`/`release`, `holdings`, `next`, `peer`, `merge-advice`, `handover`, `status`, `land`, `gc`, `doctor`, `selftest`; `--json`; prints denominators. |
| Hand-over gate | The one refusal: `awaiting_review`/close needs recorded green at HEAD + main merged. Never blocks a prompt or a WIP commit. |
| Beads boundary | Read via `bd --json`; write only through `bd` (CAS); leases mirrored in ledger; `bd` pinned. |
| Coordinator (human-facing session) | Steers, triages, priorities/lanes, rulings, arbitration, `land`. SendMessage stays the channel. |

## Essentials

- Live adopter fleet is running on this machine (`~/projects/adopter`); never modify its state
  from here.
- Owner is `29932896+AJTJ@users.noreply.github.com`; commits are authored `ajtj`.
