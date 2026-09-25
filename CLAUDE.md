# CLAUDE.md — Air (`ai_runner`)

Rules that apply without being looked up, and indexes of where everything else lives.
Index items are 1–3 lines; detail lives behind the link.

## Rules

- **No Claude memory for this project. Ever.** It is an opaque, untrackable surface. Rules and
  decisions live here and in `docs/`. If memory files exist for this project, delete them and
  move the content into `docs/decisions.md`.
- **Source trail always.** Every research claim cites a primary source — URL with access date,
  or `path:line-range`. Claims derived from the adopter's notes cite the note *and* the source it
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
  warning. Evidence: `.claude/skills/do-less/references/evidence.md` (owner, 2026-08-21).
- **Only build what makes sense.** Nothing is built without a named pain from the record it
  removes, and it ships with a red/green probe that proves it fires. Gas Town is the cautionary
  case (`docs/design.md` §11, the Gas Town row). **Invoke the `check-resources` skill first**:
  the harness, the field, then Air, with the answer written where the work is recorded. On
  2026-08-24 a shallow pass missed eleven projects that had already built parts of Air, one of
  them the same architecture on the same task store (owner, 2026-08-24).
- **Building the projects comes first; building Air is secondary.** Air exists so the owner's
  projects get built. When a round is running, Air records and does not change; findings go to
  captures and the round log, and are reviewed in one pass when there are no
  tasks left (owner, 2026-08-21).
- **A human is always in the loop.** Core requirement, not a phase. Every agent session is a
  terminal the owner can watch and type into (three workers, a verification lane, and the coordinator); Air's
  launchers start interactive sessions, never headless ones, and nothing Air builds may take the
  owner out of the loop or hide what an agent is doing. Introspection into live state
  (`air status`, the event stream) is part of the same requirement.
- **Productive sooner than later.** Improve the adopter's current process incrementally; every early
  milestone is something the adopter can run. Prefer replacing one prose rule with one enforced
  check over designing a platform.
- **Releases are the line in the sand, and the surface version moves with them.** Air is
  installed into other repos, so "what this binary is" has to be answerable by the binary. One
  release = one appended row in `install::RELEASES` (crate version, surface version, notice
  count) + the same version in `Cargo.toml` + `make release`, which refuses a dirty tree or a
  non-main branch, runs `air release-check` then `make verify`, and tags what it verified.
  **Releases are cut per round, not per notice** (owner, 2026-09-06, air-mir): a lane appends
  its surface notice and no row; the coordinator appends one row at round end covering every
  notice since the last, and `air release-check` refuses the release until the count and the
  version agree. `make verify` refuses only a count going backwards. Nineteen releases and
  five row collisions in one day was the cost of checking at every verify. Never edit a row;
  append. **Appending the row is not the whole round-end duty; two things go with it, both the
  coordinator's and both earned on 2026-09-07** (air-wt1v, air-ilh4). **Sweep every landing of the
  round against `install::SURFACE`** and append what is missing. Two populations, two numbers,
  both from 2026-09-07 and neither interchangeable: a sweep of **66 landings** found twelve gaps
  and put the detection rate at **one in six** (air-wt1v); a separate re-check of the **12
  content-changing `roles.md` commits** in its window found **3 of 12** lacking full coverage
  (air-bh6n). Both hand-catches that round were accidents of looking for something else. No check can do it — Air can see a
  signature change and cannot see a condition's *meaning* change, which is exactly what an
  adopter needs told. And **a round-end number names the binary that produced it**: one read from
  `air status`, `air audit` or the ledger describes the **running** binary, one from `make verify`
  describes the **tree**. Those were quoted interchangeably for twelve hours while the fleet ran a
  build eleven hours behind main, which made a red undiagnosable and nearly produced a filed
  defect from a sensor that was not there. A release row's comment is **not** a notice: `air
  install` prints `SURFACE` and never `RELEASES`, so prose there reaches nobody while reading
  exactly like coverage. The reason it is enforced rather than written down is that
  forgetting fails toward *permitting* — `air install`'s downgrade refusal quietly stops
  noticing, and a stale binary shows an adopting repo none of the notices telling it to upgrade
  (owner, 2026-08-29; air-w9d). Before that ruling Air had no release concept at all: `0.0.1`
  since the first commit, no tags, and `Installed.air_version` claiming in its own doc comment
  to identify a binary it could not.
- **Machinery over Markdown.** Every rule that can be a deny rule, a hook check, a launcher flag,
  a tool schema, or an injected fact should be one; Markdown is for the *why* and for judgement
  that cannot be encoded. When a prose rule becomes machinery, delete the prose (owner,
  2026-08-20).
- **Rust.** Prefer using or borrowing from an existing good project; research must show why not
  before we build. Never make a target repo's tooling depend on Air's *build* — install a binary.
- **Steal avidly** from the adopters we work with and from `~/projects/metis` (and cite what was
  taken). **Cite them as "an adopter", never by name** (air-bpj, owner 2026-09-06: the Air
  project is separate from theirs). The incident keeps its date, its count and its `air-` bead;
  the name, their paths, their bead ids and anything that copies their files live in `private/`,
  which is ignored. `make verify` runs `air adopter-check`, which reads the names from
  `private/adopters.md`. **That file is gitignored, so a fresh clone of this repo has none and
  the check must be turned on deliberately**: `.claude/air.json` says `"adopters": true`, and
  declared-with-no-list is a refusal rather than a skip (air-jsz). It skipped silently for a
  whole round, including on the sweep's own verify, which is why the declaration exists.
- **Tests are optimized for speed, always.** They run constantly; per-test cost is a first-class
  constraint (in-memory SQLite, temp git repos, no sleeps, no network, parallel-safe).
- **Talking to the owner.** Plain language, short. Lead with the thing the owner has to know or
  do; stop there. Default is five lines or fewer. No tables, no headers, no bold-label lists,
  no bead ids unless the owner has to act on one. Do not report each agent finishing each task;
  report when a round ends, when something is blocked, or when asked. Detail is available on
  request and is not volunteered. If the answer is "nothing needed", say that and stop. Owner,
  2026-08-22, after a status report they refused to read.
  **Being reachable is part of it**: the coordinator's context is the channel the owner and
  every worker reach, so long reads, dry runs and analyses go to a background agent with a file
  deliverable while the filing and the deciding stay with the coordinator (roles.md, Coordinator
  section; owner, 2026-09-06, air-zth).
- **A claim that crosses between projects is checked by the receiver before it is acted on.**
  Not hedged harder by the sender: a derived statement and an observed one have identical
  grammar, and the derivation leaves no trace in the sentence. Open the file, run the `--help`,
  read the line cited. Applies in both directions and to commands most of all. **Check even
  when you agree. Agreement is when checking feels least necessary and is most valuable.**
  Owner, via the 2026-08-22 exchange with an adopter's coordinator: three corrections, all caught by the
  receiver opening the file, none by the sender flagging; and a fourth that both sides held and
  neither checked, plausibly *because* the other had said it.
  **The same rule pointed inward is the `project-diligence` skill**: invoke it before stating a
  number about this repo, before claiming a mechanism fires or is shipped, and before saying what
  the installed `air` does (owner, 2026-08-29, air-476).
- **This file** is rules + indexes + essentials only. What is still to build goes in `docs/design.md`
  §10, technology choices in its §11, framing and rulings in `docs/decisions.md` (owner, 2026-09-25).

## Index — documents

| Read when | Document |
|---|---|
| What Air is today: components, interfaces, data, flows, invariants, failure model, operations, the TODO list in §10 and the technology decisions in §11 (the one design record: there is no plans or research directory; edit it in the same change that alters what it describes, skill `design-doc`) | [`docs/design.md`](docs/design.md) |
| Wanting the "why", framing, and every owner decision (dated, append-only) | [`docs/decisions.md`](docs/decisions.md) |
| Orienting in `docs/` | [`docs/README.md`](docs/README.md) |
| Changing the fleet's shape (who merges, which checkout a role works in, how branches reach main, what each role may do) | skill `system-design`; the target shape and its open rulings are `docs/design.md` §10, the merge-queue prior art `.claude/skills/system-design/references/verification-lane.md` |
| Why Rust, SQLite, bd, Claude Code, worktrees, tmux, the MCP channel and the merge-queue lane; whether a part of Air is now commodity | `docs/design.md` §11 (technology decisions, sourced); the field roster is `.claude/skills/check-resources/references/field.md` |
| What Claude Code itself provides and can confine (live inventory, dated; hook events; edge cases) | `.claude/skills/check-resources/references/harness-facts.md` |
| Working with `bd`: versions, the ready and claim semantics, the dependency guard, whether to replace it | skill `beads`; `.claude/skills/beads/references/bd-facts.md` |
| The findings Air's rules rest on: guardrails as throttles, corpus principles, verified numbers and claims not to repeat | `.claude/skills/do-less/references/evidence.md` |
| Integrating Air into a target repo (install, rules to change, upgrading) | [`docs/rules/adopting-air.md`](docs/rules/adopting-air.md) |
| Which role an agent is and what it may do (shipped to adopters as `.air/roles.md`) | [`docs/rules/roles.md`](docs/rules/roles.md) |
| Decomposing a feature, sizing beads, epic and bead states | skills `decomposition`, `phase-transitions` |
| Porting or writing a skill | [`.claude/skills/PROVENANCE.md`](.claude/skills/PROVENANCE.md); every skill carries a `## Provenance` footer |
| Writing prose, docs, commits, PRs, tests, reviews | skills `plain-language`, `writing-readmes`, `writing-style`, `writing-docs`, `commits`, `writing-pr-descriptions`, `writing-rust-tests`, `review`, `rust-safety`, `beads`, `parallel-worktrees` |
| About to state a number, a rate, or what the installed `air` does | skill `project-diligence` |
| What a round left behind (session journals, round logs, per-bead digests; records, not reading) | [`docs/journal/`](docs/journal/), [`docs/digests/`](docs/digests/) |

## Index — systems and subsystems

One line each; `docs/design.md` §3 to §6 is the full description and is the one kept current.

| System | One line |
|---|---|
| `crates/ledger` (`air-ledger`) | SQLite WAL ledger at the main checkout (`.air/ledger.db`) plus NDJSON events (`.air/events/`). Rows have no time-based expiry. |
| `crates/hooks` (`air-hooks`) + `air hook` | Hook I/O types, the pure hand-over gate, the worktree fence, the edit journal; `air hook` dispatches every harness event, fails open, and writes one event line per invocation. The gate refuses only with `AIR_ENFORCE=1`. |
| `crates/bd` (`air-bd`) | The `WorkLedger` trait over `bd --json`; never called from a hook path. |
| `crates/cli` (`air`) | Every command (`air --help`), the MCP server, the launchers, install and init, status and its attention conditions, landing, audit, and the self-test. |
| `air mcp` | One stdio MCP server: the coordinator's channel (attention conditions pushed from a ledger poll) plus tools and resources that invoke the CLI with `--json`. |
| Launchers `air worker <name>` / `air coordinator` | Interactive `claude` in a worktree Air made (or the main checkout for the coordinator), roles prose appended, a deny list that holds in every permission mode, role env on the process, a named tmux session on request. |
| Hand-over gate | The one refusal: closing a bead needs a recorded green at a commit containing `main`, a claim or trailer, and a tracked digest. Never blocks a prompt or a WIP commit. |
| Coordinator (human-facing session) | Steers, triages captures, files and prioritises beads, lands. Informed by the channel, never woken by cron. `SendMessage` is the agent-to-agent channel and every message is recorded. |

## Essentials

- **A session may act only on its own project. Talking to another one is fine.** Another
  project's worktrees, tmux sessions and workers are never ours to kill, restart, re-model or
  tidy; reading them and messaging them is encouraged, and the cross-project channel is how
  three wrong claims were caught on 2026-08-22. `AIR_PROJECT` on both launchers records which
  fleet a session belongs to, and `air --repo` outside this checkout is refused. The PreToolUse
  denial of a `tmux` command naming another project's session (air-0lk, narrowed by air-3oq)
  fired zero times ever and was deleted on 2026-08-29 (air-9u6), so on the tmux half this line
  IS the rule rather than a description of a check. Other fleets run on this machine
  (`the adopter's checkout`).
- Owner is `29932896+AJTJ@users.noreply.github.com`; commits are authored `ajtj`.
- Green means `make verify` (fmt, clippy, tests, `air selftest` on this tree's build); record it
  with `air record verify -- make verify` (owner, 2026-08-22).

## This repo's work flow

The fleet protocol — both closing sequences, the lane's loop, landing, proof — is Air's and
lives in [`docs/rules/roles.md`](docs/rules/roles.md), shipped as `.air/roles.md` (owner,
2026-09-25). What is this repo's own:

- **Verify** is `make verify`, recorded as `air record verify -- make verify`. There is no
  precheck and no test-state reset.
- **The lane**, when one runs, is `verify_lane` in `.claude/air.json`; absent, no lane runs.
- **Digests** go in `docs/digests/YYYY-MM-DD-<worker>-<bead>.md`, open with front matter
  `---` / `bead: <id>` / `---`, and are committed (the gate reads the declared bead, air-agq,
  and refuses an untracked file, air-ahl).
- **Journals** go in `docs/journal/<session>.md` (`journal_dir`): a bug you hit, a wrong turn,
  a claim you later found wrong. Not the digest, not a capture; nothing reads them (air-3xww).
- **Proof counts** are quoted from the run you just did, never from a doc: a sentence here once
  carried `27 probes` against a real 34 (air-jc0).
