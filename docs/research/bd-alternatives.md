# What a replacement for `bd` would have to provide, and what Air touches in `bd` today

Research note, air-6hl, worker ledger, 2026-09-06. Owner, the same day: *"I think we can do some
research. But our project is currently integrated quite deeply."*

**Research only. No code changed.** Every number below was re-derived on 2026-09-06 from this
tree or from a live API call, not copied from the earlier notes; where an earlier note said
something different, the difference is stated.

Prior art this builds on: [`beads-and-gastown.md`](beads-and-gastown.md) §0 and §1.9 (2026-08-17/18)
and [`harness-and-orchestrator-landscape.md`](harness-and-orchestrator-landscape.md).

---

## 0. The finding that reframes the question

**`bd v1.3.0-rc.1` shipped on 2026-08-31, two weeks after the research this bead was filed
against, and it restores everything Air worked around.** Source: the GitHub releases API
(`https://api.github.com/repos/steveyegge/beads/releases/tags/v1.3.0-rc.1`, fetched
2026-09-06), quoting its own body:

- **Work leases.** *"A claim used to be permanent: a worker that died mid-task stranded its
  bead `in_progress` forever with no recovery verb. Claims now carry a lease -
  `lease_expires_at` (default TTL 5m) and `heartbeat_at`"*, with `bd heartbeat`, `bd reclaim`
  and `bd unclaim`.
- **Compare-and-set.** *"`bd update --if-assignee` / `--if-status` apply only if the bead's
  current value still equals the expected one - one atomic transaction, nothing written on a
  mismatch."*
- **`bd serve`.** *"One process answering 41 OpenAPI-specified operations across 35 paths
  instead of a `bd` subprocess forked per call."*
- **Events journal.** *"Every committed bead mutation writes one ordered record in the same
  transaction as the mutation."*
- **Storage.** *"Dolt is the only storage backend; SQLite, PostgreSQL and MySQL are gone."*
- **Migration cost.** *"the first invocation after installing migrates your schema in place -
  13 main-series migrations plus 15 clone-local ones, about 28 migrations."*

The stable release is still **v1.2.2 (2026-08-15)**, which is v1.1.2 code and states in its own
notes that *"The 1.2.x-only features (work leases, the events journal, sync federation, the HTTP
API server, provenance events) are not in this release"* (releases API `/latest`, fetched
2026-09-06). Locally: `bd --version` → `bd version 1.2.2 (Homebrew)`.

So the two things that would most change Air's relationship to bd are the *same upstream
release*, not a replacement: **`bd serve` removes the per-process cost, and CAS removes Air's
reason to own the claim check.** Both are in an rc, not a release, and Air has been burned by an
untested bd release once already (§0 item 2 of the earlier note: 1.2.1 was published by accident
and migrated the schema v53→v65).

**Nothing to do today, and one thing to watch:** when v1.3.0 leaves rc, re-read §3 and §4 of
this document, because two of the three properties Air had to build for itself would then be
upstream's.

---

## 1. The exact `bd` surface Air uses

Air's whole dependence is `crates/bd/src/lib.rs` (**397 lines**), which is one trait
(`WorkLedger`, 10 methods) and one implementation (`BdCli`, shell-out to `bd --json`).

### 1.1 Through the trait (`crates/bd/src/lib.rs:305-363`)

| Method | argv | Source |
|---|---|---|
| `ready` | `bd ready --json` | `lib.rs:307` |
| `in_progress` | `bd list --status in_progress --json` | `lib.rs:311` via `by_status` |
| `by_status` | `bd list --status <s> --json` | `lib.rs:315` |
| `show` | `bd show <id> --json` | `lib.rs:319` |
| `show_all` | `bd show <id> <id> … --json` | `lib.rs:332-336` |
| `claim` | `bd update <id> --claim --actor <a>` | `lib.rs:340` |
| `set_status` | `bd update <id> -s <status>` | `lib.rs:345` |
| `reopen_unassigned` | `bd update <id> -s open -a ""` | `lib.rs:151` (`reopen_argv`) |
| `comment` | `bd comment <id> <text>` | `lib.rs:355` |
| `close_all` | `bd close <id> … --reason <r> [--actor <a>]` | `lib.rs:159` (`close_argv`) |

Six distinct subcommands: `ready`, `list`, `show`, `update`, `comment`, `close`. Flags:
`--json`, `--status`, `--claim`, `--actor`, `-s`, `-a`, `--reason`.

### 1.2 Outside the trait

| Call | argv | Where | Why |
|---|---|---|---|
| version gate | `bd --version` | `cmd/doctor.rs:27` | pin check against `BD_PINNED` (1.2.2) |
| scaffold | `bd init --prefix <p> --non-interactive --init-if-missing --skip-agents --skip-hooks` | `cmd/init.rs:455-466` | once, at `air init --write` |
| custom status | `bd config set status.custom awaiting_review` | `cmd/init.rs:471-478` | once; `awaiting_review` is not a bd default |

Nine subcommands in total, and three of them run once in a repo's life.

### 1.3 What Air reads out of a bead

`air_bd::Issue` (`lib.rs:86-113`), 12 fields: `id`, `title`, `description`,
`acceptance_criteria`, `status`, `priority`, `assignee`, `labels`, `parent`, `created_at`,
`updated_at`, `issue_type`. `#[serde(default)]` on the struct, so unknown bd fields are ignored
and missing ones default. Against bd's own schema (`bd schema`, ~60 issue fields per
`beads-and-gastown.md` §1.2), **Air reads about a fifth of the record and writes four fields.**

### 1.4 What Air deliberately does NOT use

This is the shorter list and it is the load-bearing one, because a replacement does not have to
provide it:

- **No `bd` leases, heartbeats or CAS.** Air owns both (`claims` table, `air lease`), by
  decision: *"CAS and leases are owned by Air's ledger because bd 1.2.2 has neither"*
  (`lib.rs:3-5`). `air claim` checks CAS in Air's ledger **before** calling `bd update --claim`
  (`lib.rs:135-137`).
- **No `bd dep`, `blocked`, `recompute-blocked`, `epic`, `graph`.** The module doc lists them as
  "verified present" but no call site invokes them. Air reads the ready list bd computes and
  never manipulates the graph.
- **No `bd create`.** Air never files a bead; the coordinator does, by hand, with
  `bd create --validate --estimate N`. `--validate` is bd's, invoked by a person, and appears in
  Air only as prose (`cmd/capture.rs:4`, `cmd/mcp.rs:284`). `Bash(bd create *)` is on the worker
  deny list (`cmd/launch.rs:40`).
- **No `bd sync`, `serve`, `events`, `remember`/`recall`, `prime`, `hooks`.** `air init` passes
  `--skip-agents --skip-hooks` precisely to keep bd's own agent prose and git hooks out.
- **No `bd` MCP server.** Air's `air mcp` is its own.
- **`Bead: <id>` trailers are Air's, not bd's.** Attribution reads a git trailer and asks bd
  nothing (air-7kp).

### 1.5 What it costs, measured here today

From `air audit` on this repo's ledger, 2026-09-06:

    latency: bd median 1429 ms per process over 751673 process(es);
             `air status` waits 5716 ms for one
    latency: `air status` p50 3902 ms, p90 5027 ms, p99 6317 ms, max 26601 ms over 31093 run(s)

**The cost is per process, not per query** (air-869, re-confirmed by the median holding across
`ready`, `list` and `show`). `air status` is seconds because it forks a handful of them. The
store on this repo is **50 MB** (`.beads/embeddeddolt` 34 MB, `.beads/backup` 16 MB), against an
adopter's 2.1-2.2 GB.

Nothing bd is called from a hook path; the Stop hook reads `.air/ready.json`, a cache written by
the commands that already paid for a `bd ready` (`cmd/ready_cache.rs:1-14`), because the hook
budget is 100 ms and `bd ready --json` is ~0.7-1.4 s.

---

## 2. The properties a replacement must have

Derived from §1, not from a wish list. Each names what breaks without it.

1. **A dependency-aware ready list.** `bd ready` is the one query Air cannot compute itself: it
   is the whole reason bd is here rather than a table. Without it Air's `ready:` line, the Stop
   nudge and `idle-without-claim` have no input.
2. **An atomic claim, or a claim Air can guard.** Air already owns CAS, so the weaker
   requirement holds: a claim write that is idempotent and reports failure. bd 1.2.2 does not
   have CAS and Air works anyway.
3. **JSON on every read.** `--json` on `ready`, `list`, `show`. Air parses bd's output and
   tolerates unknown fields; a replacement whose JSON differs in field names costs one serde
   struct.
4. **Custom statuses.** `awaiting_review` is not a bd default and Air's `air init` declares it.
   Survives only on beads that already carry it, but the gate still matches it.
5. **Issue types.** `epic` has to be distinguishable from `task`, since air-f10 excludes epics
   from the claimable count and `air claim <epic>` is refused.
6. **Validated description sections.** `bd create --validate` refuses a bead with no
   `## Acceptance Criteria` (per type). This is the coordinator's gate, not Air's, but a repo
   that loses it loses `air land`'s acceptance print, which reads those sections.
7. **Single machine, no daemon, no network.** Air's whole model is one machine, a few
   worktrees, and hooks that must answer in 100 ms. A tracker needing a running server is a new
   failure mode in the middle of the fleet. (`bd serve` in 1.3.0-rc.1 is a *daemon*, which is a
   real tension with this property and the reason it is not a free win.)
8. **A CLI, or a Rust library.** Air shells out today. A library would remove the per-process
   cost, which is the single largest number in §1.5.

Two properties Air does **not** need, and it matters: **distributed sync** (one machine) and
**graph editing** (Air reads the graph, never writes it).

---

## 3. Candidates

### 3.1 Stay on `bd` 1.2.2 (the status quo)

**Verdict: correct today.** It satisfies every property in §2 except 8's second half, and the
per-process cost is paid outside hooks. The known defects are on file and all have workarounds
already built: the assignee lock on open beads in 1.2.x (`reopen_unassigned`, air-0kk, one
process instead of two), `bd show --json` dropping comment text and `bd list --json` omitting
closed beads (round note items 47-48), `bd ready` excluding custom statuses (upstream #5831).
**Cost of staying: zero. Risk: bd is one author's 225k-line Go project whose last stable release
is a retraction of the one before it.**

### 3.2 `bd` 1.3.0 when it leaves rc

**Verdict: watch, do not adopt while it is an rc.** It gives Air CAS and a single serving
process, which are §2 items 2 and 8. Against it: 28 in-place schema migrations on first
invocation, a daemon in tension with §2 item 7, and the 1.2.1 precedent (published by accident,
migrated the schema, retracted four days later). The re-read trigger is the stable tag, not the
rc.

### 3.3 `Dicklesworthstone/beads_rust` (`br`)

Rust. **1,082 stars, v0.5.10, last pushed 2026-09-06** (GitHub API and the repo's README,
fetched 2026-09-06 — up from 1,052 stars and v0.3.2 at the earlier note's 2026-08-15, so it is
actively maintained, not a freeze). It has, from its own README: `br ready` returning "only
unblocked, actionable issues"; `br update --claim` with exclusive locking; custom statuses via
`policy.yaml`; *"Every command supports `--json` for AI coding agents"*; `br lint` for missing
template sections; full `dep add/remove/list/tree/cycles`.

That is §2 items 1-6, and it is SQLite rather than Dolt, which is §2 item 7 without a daemon.

**Verdict: the only real alternative, and still not worth a swap today.** Three reasons, each
from its own README:

- **Store-incompatible and wire-incompatible with `bd`.** *"Its length-prefixed content hashes
  deliberately differ from classic bd hashes. Use JSONL interchange rather than assuming the two
  tools can share an identical live database schema."* A migration is an export/import, not a
  point at the same directory, and every repo's store would be a one-way move.
- **Binary only, no library crate.** The build yields one `br` executable. So Air would still
  shell out and still pay a per-process cost, only a smaller one. **The largest number in §1.5
  is not fixed by this candidate**, which removes most of the reason to move.
- **It is a fork of an architecture upstream abandoned.** Its own words: the SQLite+JSONL
  architecture *"is being replaced with approaches better suited to Steve's vision."* Adopting
  it is a bet against upstream, taken by a repo with no measured pain from upstream.

### 3.4 `delightful-ai/beads-rs`

**24 stars, 38 open issues, last pushed 2026-08-19** (GitHub API, 2026-09-06 — unchanged in
three weeks). A git-refs redesign, alpha, missing mail/multi-repo/compaction per the earlier
note. **Verdict: no.** Not on maturity, and this one is unambiguous.

### 3.5 A plain SQLite table of Air's own

Air already runs a SQLite ledger with `claims`, `sessions`, `landings` and eight other tables.
Adding an `issues` table is mechanically easy.

**Verdict: no, and this is the one worth arguing.** What Air would be building is §2 item 1 —
a dependency graph with a correct ready-list query, cycle detection, and the epic/child
semantics air-f10 depends on — plus item 6's validation and a CLI a person can drive by hand.
That is the whole of a work tracker, and the `do-less` rule's question is what recorded failure
it removes. There is none: the ledger has no row saying bd cost this repo a bead, an hour or a
wrong answer. It would remove 1.4 s per process, which is not paid in any hook, and it would
make the coordinator's `bd create --validate`, `bd dep` and every bd habit Air deliberately left
alone into Air's problem. **This is the Gas Town shape** (`beads-and-gastown.md` §2.5): a
framework growing toward completeness rather than toward the owner's projects getting built.

If bd ever became unavailable rather than merely slow, this is the fallback, and §4 says what it
would cost.

---

## 4. What the swap costs, behind `WorkLedger`

The boundary is already one trait, which is what makes this answerable in numbers.

**What a new backend has to write:**

| Piece | Lines |
|---|---|
| a second `impl WorkLedger` (10 methods) | ~150-250, by analogy with `BdCli`: 58 lines of `impl WorkLedger` (`lib.rs:305-363`) over 52 lines of `impl BdCli` (`185-237`) and 38 of `wait_drained` (`245-283`) |
| its own `Issue` deserialisation, if the JSON differs | ~0-40 (the struct is `#[serde(default)]`; renames are attributes) |
| the ready-list query, if the backend does not compute it | the whole of §3.5, not a line estimate |

**What has to change in `crates/cli`:** 5 files name the concrete type `BdCli` (13 mentions,
11 outside `selftest.rs`):

| File | `BdCli` mentions | What it does with it |
|---|---|---|
| `cmd/claim.rs` | 5 | `bd_for()` builds it; `probe_bd` clones it with a shorter timeout |
| `cmd/status.rs` | 4 | two function signatures, plus `.timeout`/`.label` per call site |
| `cmd/ready_cache.rs` | 2 | one signature, plus `.timeout`/`.label` |
| `cmd/budgets.rs` | 1 | a catalogue string naming `BdCli::new` as the budget's site |
| `cmd/selftest.rs` | 2 | probes |

Ten files in `crates/cli/src/cmd` `use air_bd` at all. Four of them (`status`, `ready_cache`,
`claim`, `selftest`) also name the concrete type; the other six (`capture`, `doctor`, `close`,
`land`, `metis`, `mod`) touch only `Issue` and `WorkLedger` and do not change, because they are
already written against the trait. `budgets.rs` imports nothing from `air_bd` and names `BdCli`
once, inside a catalogue string.

**The honest estimate: 1 new file of 150-250 lines, and about 15 edited lines across 5 files**
to make `bd_for` return the new type (or `Box<dyn WorkLedger>`, which costs three more mentions
where `.timeout` and `.label` are set on the concrete struct — those are per-call-site budget
overrides and would need a trait method or a builder). Call it **a day**, if and only if the
replacement supplies the ready list. If it does not, §3.5's estimate is the real one and it is
not a day.

Two things the trait does not cover and a swap would have to answer separately:

- **The three one-shot `air init` calls** (`bd init`, `bd config set status.custom`,
  `bd --version`) are direct `Command::new` calls in `cmd/init.rs` and `cmd/doctor.rs`, outside
  `WorkLedger`. Roughly 30 lines.
- **`BD_PINNED`** and the pin check. One constant, one comparison, and a surface notice.

---

## 5. Verdict

**Stay on `bd`. Change nothing. Watch one thing.**

The integration is deep in the sense the owner meant — nine subcommands, ten trait methods, 751k
recorded processes — but it is deep through **one 397-line file and one trait**, and that is the
shape that makes it cheap to leave. The measured cost of staying is 1.4 s per process, paid
outside every hook path, in a repo whose ledger records no failure caused by bd.

The one thing to watch is **`bd` v1.3.0 leaving rc**. It brings CAS and a one-process server,
which are two of the three properties Air built or worked around for itself. When it does, the
questions to re-ask are: does `bd serve` remove enough of §1.5's cost to be worth a daemon
inside the fleet (§2 item 7), and does upstream CAS let Air's `claims` table stop being the
authority on who holds a bead. Neither is answerable against an rc.

**Removal condition for this document:** when bd 1.3.0 is stable and those two questions are
answered, or when the ledger records a first failure attributable to bd. Either way this note is
re-derived, not amended: the numbers in it have moved once already in three weeks.
