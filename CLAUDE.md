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
  case (`docs/research/beads-and-gastown.md §2.5`). **Invoke the `check-resources` skill first**:
  the harness, the field, then Air, with the answer written where the work is recorded. On
  2026-08-24 a shallow pass missed eleven projects that had already built parts of Air, one of
  them the same architecture on the same task store (owner, 2026-08-24).
- **Building the projects comes first; building Air is secondary.** Air exists so the owner's
  projects get built. When a round is running, Air records and does not change; findings go to
  `docs/notes/air-backlog.md` and the round log, and are reviewed in one pass when there are no
  tasks left (owner, 2026-08-21).
- **A human is always in the loop.** Core requirement, not a phase. Every agent session is a
  terminal the owner can watch and type into (today: one coordinator + three workers); Air's
  launchers start interactive sessions, never headless ones, and nothing Air builds may take the
  owner out of the loop or hide what an agent is doing. Introspection into live state
  (`air status`, the event stream) is part of the same requirement.
- **Productive sooner than later.** Improve adopter's current process incrementally; every early
  milestone is something adopter can run. Prefer replacing one prose rule with one enforced
  check over designing a platform.
- **Releases are the line in the sand, and the surface version moves with them.** Air is
  installed into other repos, so "what this binary is" has to be answerable by the binary. One
  release = one appended row in `install::RELEASES` (crate version, surface version, notice
  count) + the same version in `Cargo.toml` + `make release`, which refuses a dirty tree or a
  non-main branch, runs `make verify`, and tags what it verified. **Appending a surface-change
  notice forces a release**: the notice count stops matching and `make verify` fails until a row
  is added. Never edit a row; append. The reason it is enforced rather than written down is that
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
- **Steal avidly** from `~/projects/adopter` and `~/projects/metis` (and cite what was taken).
- **Tests are optimized for speed, always.** They run constantly; per-test cost is a first-class
  constraint (in-memory SQLite, temp git repos, no sleeps, no network, parallel-safe).
- **Talking to the owner.** Plain language, short. Lead with the thing the owner has to know or
  do; stop there. Default is five lines or fewer. No tables, no headers, no bold-label lists,
  no bead ids unless the owner has to act on one. Do not report each agent finishing each task;
  report when a round ends, when something is blocked, or when asked. Detail is available on
  request and is not volunteered. If the answer is "nothing needed", say that and stop. Owner,
  2026-08-22, after a status report they refused to read.
- **A claim that crosses between projects is checked by the receiver before it is acted on.**
  Not hedged harder by the sender: a derived statement and an observed one have identical
  grammar, and the derivation leaves no trace in the sentence. Open the file, run the `--help`,
  read the line cited. Applies in both directions and to commands most of all. **Check even
  when you agree. Agreement is when checking feels least necessary and is most valuable.**
  Owner, via the 2026-08-22 ai_runner/adopter exchange: three corrections, all caught by the
  receiver opening the file, none by the sender flagging; and a fourth that both sides held and
  neither checked, plausibly *because* the other had said it.
  **The same rule pointed inward is the `project-diligence` skill**: invoke it before stating a
  number about this repo, before claiming a mechanism fires or is shipped, and before saying what
  the installed `air` does (owner, 2026-08-29, air-476).
- **This file** is rules + indexes + essentials only. Plans, framing, and decisions go in `docs/`.

## Index — documents

| Read when | Document |
|---|---|
| Wanting the "why", framing, and every owner decision (dated) | [`docs/decisions.md`](docs/decisions.md) |
| Orienting in the research | [`docs/README.md`](docs/README.md) — index of all reports |
| Wondering whether to switch to another harness or orchestrator, or whether a part of Air is now commodity | [`docs/research/harness-and-orchestrator-landscape.md`](docs/research/harness-and-orchestrator-landscape.md) |
| The full inventory of everything Air ships: every command, table, hook, MCP tool, integration, env var and mechanism, with why each exists, what already does it, and a verdict (living doc, re-run the counts) | [`docs/plans/0007-surface-audit.md`](docs/plans/0007-surface-audit.md) |
| What Claude Code itself already provides (live inventory, dated, with the limits that matter) | [`docs/research/claude-code-control-surfaces.md`](docs/research/claude-code-control-surfaces.md) §0 |
| Deciding what shape Air is and why | [`docs/research/SYNTHESIS.md`](docs/research/SYNTHESIS.md) |
| Building on the first slice (ledger facts, hooks, the one refusal, evidence weighting) | [`docs/plans/0001-first-slice.md`](docs/plans/0001-first-slice.md) |
| The first-round surface as built (claims wrap bd, capture/triage, status/attention, `air mcp` channel, install, launchers) and how to operate it on adopter | [`docs/plans/0004-first-round-surface.md`](docs/plans/0004-first-round-surface.md) |
| Integrating Air into a target repo (install, rules to change, self-maintenance) | [`docs/rules/adopting-air.md`](docs/rules/adopting-air.md) |
| What comes next, in order | [`docs/plans/0005-roadmap.md`](docs/plans/0005-roadmap.md) |
| The whole change list from the 2026-08-24/25 audit and adopter's round logs, ruled item by item, with a path for each | [`docs/plans/0008-consolidated-changes.md`](docs/plans/0008-consolidated-changes.md) |
| What the adopter round proposes, and the do-less verdict on each | [`docs/plans/0006-post-round-changes.md`](docs/plans/0006-post-round-changes.md) |
| Which role an agent is and what it may do | [`docs/rules/roles.md`](docs/rules/roles.md) · research: [`docs/research/agent-roles-and-confinement.md`](docs/research/agent-roles-and-confinement.md) |
| Decomposing a feature, sizing beads, cutting per-worker queues | skills `decomposition`, `phase-transitions`; research: [`docs/research/metis-decomposition-and-agile.md`](docs/research/metis-decomposition-and-agile.md) |
| Which metrics Air records (the single list) | [`docs/research/verification/ticks/2026-08-18-0430-measurement-spec.md`](docs/research/verification/ticks/2026-08-18-0430-measurement-spec.md) |
| Running the work procedure: capture → triage → bead, decomposition, dispatch, hand-over, landing (the single procedure; what Air enforces vs judgement) | [`docs/plans/0002-what-to-work-on.md`](docs/plans/0002-what-to-work-on.md) |
| Porting or writing a skill | a private skills inventory; ported skills live in `.claude/skills/` with a `## Provenance` footer each and an index in [`.claude/skills/PROVENANCE.md`](.claude/skills/PROVENANCE.md) |
| Writing prose, docs, commits, PRs, tests, reviews | Use the skills: `plain-language` (length budgets; shorter wins), `writing-style`, `writing-docs`, `commits`, `writing-pr-descriptions`, `writing-rust-tests`, `review`, `rust-safety`, `beads`, `parallel-worktrees` — see `.claude/skills/` |
| About to state a number, a rate, or what the installed `air` does | skill `project-diligence` — re-derive rather than re-read, check the binary against the repo, confirm the probe was seen failing |
| Rust conventions (errors, lints, MSRV — open decisions) | [`docs/plans/0003-rust-conventions.md`](docs/plans/0003-rust-conventions.md) |
| Worktree protocol for this repo | [`docs/rules/worktree-protocol.md`](docs/rules/worktree-protocol.md) · [`docs/rules/writing.md`](docs/rules/writing.md) |
| Touching billing/cost assumptions | [`docs/research/claude-code-billing.md`](docs/research/claude-code-billing.md) — primary sources only |
| Working with `bd` (versions, leases trap) | [`docs/research/beads-and-gastown.md`](docs/research/beads-and-gastown.md) §0 |

## Index — systems and subsystems (first slice 2026-08-18, plan 0001; first-round surface 2026-08-20, plan 0004)

| System | One line |
|---|---|
| `crates/ledger` (`air-ledger`) | SQLite WAL ledger at the main checkout (`.air/ledger.db`) + NDJSON events (`.air/events/`): `verify_runs`, `edit_journal`, `claims`, `sessions` (with `role`), `landings`, `captures` (schema v2). No time-based expiry. **Built.** |
| `crates/hooks` (`air-hooks`) + `air hook` | Hook I/O types, the pure hand-over gate, edit journal; `air hook` dispatches SessionStart/PreToolUse/PostToolUse/PermissionRequest/Stop/SessionEnd, fails open, ~100 ms, **one event line per invocation** (transitions included). Advisory unless `AIR_ENFORCE=1`. **Built.** |
| `crates/cli` (`air`) | Built: `record` (command/duration/bytes/dirty, suspicious flags), `handover` (4 checks incl. digest), `holdings`, `claim`/`release` (wrap `bd --claim`), `capture [--for owner]`/`inbox [--owner]`/`triage`, `lease take|release|status|break|beat`, `status [--attention]`, `mcp`, `init` (gate + everything), `install`, `worker [name] [--tmux --task]`/`coordinator` (+ `.claude/air.json` deny patterns), `close`, `land [--all]`, `hook`, `doctor`, `selftest` (probe count is whatever it prints; this line used to carry a copy
of it and was stale by 14, air-jc0). Next (after round-one data): `next`, `peer`, `merge-advice`, `gc`, PreCompact re-inject. |
| `air mcp` | One stdio MCP server: the coordinator's **channel** (pushes attention conditions from a ledger poll; no sockets, no timers) plus tools (`air_*`) and resources (`air://status`, …) that invoke the CLI. Synchronous, bounded, panic-isolated. **Built.** |
| Launchers `air worker <name>` / `air coordinator` | Interactive `claude` with native worktree isolation, roles prose appended, deny list that holds in every permission mode, env instead of drifting files; coordinator gets the channel. **Built.** |
| Hand-over gate | The one refusal: `awaiting_review`/close needs recorded green at HEAD + main merged. Never blocks a prompt or a WIP commit. |
| `crates/bd` (`air-bd`) | `WorkLedger` trait + `bd --json` shell-out (bd 1.2.2 surface); CAS/leases live in the ledger; never called from a hook. **Built (minimal).** |
| Coordinator (human-facing session) | Steers, triages the capture inbox, builds per-worker queues in beads fields, rulings, arbitration, `land`. Informed by the Air channel, not woken by cron. SendMessage stays the agent-to-agent channel. |

## Essentials

- **A session may act only on its own project. Talking to another one is fine.** Another
  project's worktrees, tmux sessions and workers are never ours to kill, restart, re-model or
  tidy; reading them and messaging them is encouraged, and the cross-project channel is how
  three wrong claims were caught on 2026-08-22. `AIR_PROJECT` on both launchers records which
  fleet a session belongs to, and `air --repo` outside this checkout is refused. The PreToolUse
  denial of a `tmux` command naming another project's session (air-0lk, narrowed by air-3oq)
  fired zero times ever and was deleted on 2026-08-29 (air-9u6), so on the tmux half this line
  IS the rule rather than a description of a check. Other fleets run on this machine
  (`~/projects/adopter`).
- Owner is `29932896+AJTJ@users.noreply.github.com`; commits are authored `ajtj`.
- Green means `make verify` (fmt, clippy, tests, `air selftest` on this tree's build); record it
  with `air record verify -- make verify` (owner, 2026-08-22).

## This repo's work flow

How a finished bead is handed on is **this repo's** choice, not Air's, and it lives here
because `.air/roles.md` deliberately does not say it (air-8zu). Air states what it records and
what it refuses; the sequence is ours.

**A worker closes its own bead with proof** (owner, 2026-08-22, air-7o3). No `awaiting_review`,
no waiting for review. Two variants; **which one is in force is the `verify_lane` key in
`.claude/air.json`**: `"verify_lane": "<worker>"` names the lane and puts variant B in force
for everyone, and an absent key means variant A. Air reads neither variant; it refuses the same
thing under both (a close needs a green at a commit containing `main`, at HEAD or at a verified
descendant, air-80x.1).

**Variant A, no verify lane** (`verify_lane` absent):

    air claim <id> [--files a,b]
    … implement; write the digest (docs/digests/YYYY-MM-DD-<worker>-<bead>.md) and commit it
    …   it opens with front matter naming the bead:  ---\n bead: <id>\n ---
    git merge main
    air record verify -- make verify        # last, so the green is at the commit containing main
    bd close <id> --reason "<proof>"
    … next bead

**Variant B, a verify lane runs** (`"verify_lane": "<worker>"`; owner, 2026-09-05, air-80x).
The worker does **not** run verify: the batch only forms if workers stop verifying individually,
which is what adopter's 2026-08-29 round learned by parking a lane whose batch never came.

    air claim <id> [--files a,b]
    … implement; write and commit the digest as above, with a `Bead: <id>` trailer on the work
    git merge main                          # your branch now shows in `air status` as batch-ready
    … wait for the lane's green; `air handover` says when the close would pass
    bd close <id> --reason "<proof: the lane's green at <sha>, which contains your commits>"
    … next bead; a commit made after the lane cut its batch waits for the next batch

The lane's own sequence, and it holds no bead while it batches: read `air status` (the
`batch-ready:` lines, or `--json` `batch_ready`), `git merge main` plus every batch-ready branch
at the sha listed, `air record verify -- make verify`, then signal the coordinator with the
members; a branch that conflicts is dropped from the batch and named to its worker, never
resolved by the lane. The coordinator lands the batch with `air land --worker <lane>`, which
attributes every bead the range names. A red batch lands nothing and is reported by member.

The digest's front matter is what the hand-over gate reads (air-agq). It used to find a
digest by looking for the worker's name in a filename and an mtime newer than the claim, which
accepted a digest written for a different bead, and accepted `touch` on an old one. A gate that
guards fails toward permitting, so it now wants the bead declared rather than guessed.

**Proof is a command and its output, a `file:line`, or a passing test.** Not a description of
the approach — "refactored the parser" is not proof; `make verify` green at `<sha>`, with the
probe count your own run printed, is. The owner's words: *"the explanation should be proof, not
verbosity. Very clear proof."* Quote the count from the run you just did, never from here: this
sentence carried `27 probes` against a real 34 for long enough that two lanes could have copied
it (air-jc0).

**If part of a bead needs the owner, close what you did and file a standalone successor bead**
naming what he must do. Do not leave the bead open for the remainder: open beads get re-claimed
and re-derived by the next worker, which is the failure this avoids.

**What makes this safe rather than an honour system:** `air handover`'s gate matches `bd close`
and `bd update -s closed` as well as `-s awaiting_review` (`is_handover_command`), and worker
launches set `AIR_ENFORCE=1` by default since air-i59. So a close without a recorded green at a
HEAD containing `main` is *refused*, not advised. The proof is enforced at the moment of
closing. Run `air handover` first if you want the missing pieces named before bd refuses them.

`awaiting_review` survives only on beads that already carry it.
