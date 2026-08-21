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
- **A human is always in the loop.** Core requirement, not a phase. Every agent session is a
  terminal the owner can watch and type into (today: one coordinator + three workers); Air's
  launchers start interactive sessions, never headless ones, and nothing Air builds may take the
  owner out of the loop or hide what an agent is doing. Introspection into live state
  (`air status`, the event stream) is part of the same requirement.
- **Productive sooner than later.** Improve adopter's current process incrementally; every early
  milestone is something adopter can run. Prefer replacing one prose rule with one enforced
  check over designing a platform.
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
| Thinking about feature → epics → tasks and how agents traverse an epic | [`docs/plans/0002-what-to-work-on.md`](docs/plans/0002-what-to-work-on.md) (draft; six owner decisions in §7) |
| Porting or writing a skill | a private skills inventory; ported skills live in `.claude/skills/` with a `## Provenance` footer each and an index in [`.claude/skills/PROVENANCE.md`](.claude/skills/PROVENANCE.md) |
| Writing prose, docs, commits, PRs, tests, reviews | Use the skills: `writing-style`, `writing-docs`, `commits`, `writing-pr-descriptions`, `writing-rust-tests`, `review`, `rust-safety`, `beads`, `parallel-worktrees` — see `.claude/skills/` |
| Rust conventions (errors, lints, MSRV — open decisions) | [`docs/plans/0003-rust-conventions.md`](docs/plans/0003-rust-conventions.md) |
| Worktree protocol for this repo | [`docs/rules/worktree-protocol.md`](docs/rules/worktree-protocol.md) · [`docs/rules/writing.md`](docs/rules/writing.md) |
| Touching billing/cost assumptions | [`docs/research/claude-code-billing.md`](docs/research/claude-code-billing.md) — primary sources only |
| Working with `bd` (versions, leases trap) | [`docs/research/beads-and-gastown.md`](docs/research/beads-and-gastown.md) §0 |

## Index — systems and subsystems (first slice built 2026-08-18; see plan 0001)

| System | One line |
|---|---|
| `crates/ledger` (`air-ledger`) | SQLite WAL ledger at the main checkout (`.air/ledger.db`) + NDJSON events (`.air/events/`): `verify_runs`, `edit_journal`, `claims`, `sessions`, `landings`. No time-based expiry. **Built.** |
| `crates/hooks` (`air-hooks`) + `air hook` | Hook I/O types, the pure hand-over gate, edit journal; `air hook` dispatches SessionStart/PreToolUse/PostToolUse/PermissionRequest/Stop/SessionEnd, fails open, ~100 ms. Advisory unless `AIR_ENFORCE=1`. **Built.** |
| `crates/cli` (`air`) | Built: `record`, `handover`, `holdings`, `hook`, `doctor`, `selftest` (`--json`, denominators). Next: `claim`/`release`, `capture`, `next`, `peer`, `merge-advice`, `status`, `land`, `gc`, `install`. |
| Hand-over gate | The one refusal: `awaiting_review`/close needs recorded green at HEAD + main merged. Never blocks a prompt or a WIP commit. |
| `crates/bd` (`air-bd`) | `WorkLedger` trait + `bd --json` shell-out (bd 1.2.2 surface); CAS/leases live in the ledger; never called from a hook. **Built (minimal).** |
| Coordinator (human-facing session) | Steers, triages, priorities/lanes, rulings, arbitration, `land`. SendMessage stays the channel. |

## Essentials

- Live adopter fleet is running on this machine (`~/projects/adopter`); never modify its state
  from here.
- Owner is `29932896+AJTJ@users.noreply.github.com`; commits are authored `ajtj`.
