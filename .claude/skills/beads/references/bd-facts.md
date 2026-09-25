# beads (bd): what Air relies on, and the alternatives

This is the single reference for beads as Air uses it: what the pinned `bd` does and does not do, the ready and claim semantics Air's gate and status line depend on, the one shape of dependency bd lets through silently, what Gas Town built on the same store and what Air copied from it, and what leaving bd would cost. It lived at `docs/research/beads.md` until 2026-09-25, when `docs/research/` was retired and it moved beside the `beads` skill; the decision to use bd, pinned, is one line in `docs/design.md` §11. It was assembled on 2026-09-14 from four documents that it replaces: `docs/research/beads-and-gastown.md` (2026-08-17/18), `docs/research/bd-alternatives.md` (air-6hl, 2026-09-06), `docs/research/verification/ticks/2026-08-18-0300-bd-1-2-x-facts.md` (tick 0300, 2026-08-18) and `docs/notes/2026-09-06-bd-refuses-the-ancestor-edge.md` (air-btz, 2026-09-06). Each claim keeps the source and date it had there; where a number has moved since, the current one is stated with its own date. Nothing here was re-fetched from upstream on 2026-09-14 except what is marked so.

**Pinned version: bd 1.2.2.** `bd --version` on this machine printed `bd version 1.2.2 (Homebrew)` on 2026-09-14, and the pin is `pub const BD_PINNED: &str = "1.2.2"` at `crates/cli/src/cmd/doctor.rs:13`, compared against `bd --version` at `doctor.rs:27-37`.

## Headline findings

These are the facts that shaped Air's beads boundary. They come from `beads-and-gastown.md` §0 (2026-08-17/18) with the corrections tick 0300 made to it on 2026-08-18 folded in rather than appended.

1. **beads is Dolt-backed, not SQLite plus JSONL.** Embedded Dolt is the default (`bd init`, in-process at `.beads/embeddeddolt/`); a `dolt sql-server` mode exists for concurrent writers. `.beads/issues.jsonl` is "an export for viewers and interchange, not the source of truth or a backup." (README, https://github.com/steveyegge/beads, which redirects to `gastownhall/beads`, accessed 2026-08-17; an adopter's `.beads/metadata.json` read locally says `"backend":"dolt","dolt_mode":"embedded"`.)

2. **The 1.2.x line was published by accident, and 1.2.2 is the retraction.** v1.2.0 and v1.2.1 "were published by accident on 2026-08-11, without release testing. v1.2.2 superseded them by re-releasing the tested 1.1 line," so v1.2.2 (2026-08-15) is v1.1.2 code under a higher number. Everything 1.2.1-only is therefore absent from the pinned binary: work leases (`bd heartbeat`, `bd reclaim`), the events journal, `bd sync`, `bd serve`, provenance events, and also, per tick 0300's re-check against tag v1.1.2 in the local clone, `bd unclaim` as a whole command, the compare-and-set guards `bd update --if-assignee` and `--if-status` with their exit code 13, `bd update --force` and its anti-steal refusal, `--brief`, and `claim.pools`. Running 1.2.1 once migrated the schema v53 to v65, and 1.2.2 refuses to open such a database until the cursor is rolled back. (https://github.com/steveyegge/beads/blob/main/docs/recovery/accidental-1-2-1-release.md and https://github.com/steveyegge/beads/blob/main/CHANGELOG.md, accessed 2026-08-17; tick 0300 rows 1.2 and 2.3, 2026-08-18.)

3. **beads is Go, about 225k lines, and there is no official Rust library.** The Rust ports track either the old SQLite plus JSONL architecture (`Dicklesworthstone/beads_rust`, binary `br`) or a git-refs redesign (`delightful-ai/beads-rs`, alpha). None reads the Dolt store the current `bd` uses, so a Rust process shells out to `bd --json`. (Yegge, "Welcome to Gas Town", https://steve-yegge.medium.com/welcome-to-gas-town-4f25ee16dd04, 2026-01-01, via web.archive.org; https://github.com/Dicklesworthstone/beads_rust and https://github.com/delightful-ai/beads-rs, accessed 2026-08-17.)

4. **Gas Town is a Go orchestrator over beads that already implements most of Air's lifecycle** (sling, worktree, work, `gt done`, Refinery merge queue, close) plus LLM-agent supervision. It is tmux-centric, Claude-Code-centric, about $100 an hour at scale by its own v1.0 metrics, and its successor Gas City (`gastownhall/gascity`) decomposes it into a controller loop with pluggable runtimes. It is a reference architecture, not a runtime to adopt. (https://github.com/steveyegge/gastown, https://github.com/gastownhall/gascity, accessed 2026-08-17; the verdict is developed below.)

Two further corrections from tick 0300 that belong with the headlines: `bd ready` is `status = 'open'` only, not merely "excludes in_progress, blocked, deferred and hooked", which means custom statuses such as `awaiting_review` never appear in it (upstream #5831); and embedded Dolt's "single writer, file-locked" is a file lock held only during `bd init`, while ordinary commands serialise on the Dolt engine lock with driver backoff and a 30 s open timeout. Both are detailed in the facts section.

## Ready and claim semantics on bd 1.2.2

What Air's `ready:` line, the Stop nudge, `idle-without-claim` and `air claim` all rest on. From `beads-and-gastown.md` §1.5 (2026-08-17, `bd ready --help` and `bd update --help` run locally against 1.2.1) as corrected by tick 0300 row 4.1, which read the code at tag v1.1.2 on 2026-08-18.

The docs describe ready work as "the claimable frontier of the graph: open beads with no open blockers, excluding anything in progress, blocked, deferred, or held by a gate" (https://github.com/steveyegge/beads/blob/main/docs/core-concepts/index.md, accessed 2026-08-17). The code is narrower. The work API sets `Status: open` with the comment "Open only, not in_progress - the same set `bd list --ready` shows", so anything not literally `open` is excluded: `in_progress`, `blocked`, `deferred`, `closed`, `hooked`, the `pinned` status, and every custom status even when its category is `active`. The adopter's `awaiting_review` and `awaiting_testing` never show; this is upstream #5831, opened 2026-08-17 with a repro on 1.2.2. The SQL adds `is_blocked = 0` (a denormalised column, repaired by `bd recompute-blocked` after a pull), `pinned = 0`, `ephemeral = 0` unless `--include-ephemeral`, `issue_type NOT IN (merge-request, gate, molecule, rig, agent, role, message)` plus `--exclude-type`, `defer_until IS NULL OR <= now` and no deferred parent unless `--include-deferred`. `--parent` is recursive over descendants. The default `--limit` is 100 (`DefaultReadyLimit = 100`; `--limit 0` is unlimited). A related bug, #5832, makes `bd list --status X --ready` silently drop `--status`. (`internal/workapi/ready.go:16,43-47`, `internal/storage/sqlbuild/ready.go:16-25,93-135` in the local clone at `d1e725d9f`; https://github.com/gastownhall/beads/issues/5831 and /5832, accessed 2026-08-18.)

`bd ready --json` returns a top-level JSON array of issue objects. Keys are omit-empty, so presence varies; Air's `Issue` struct is `#[serde(default)]` for that reason (`crates/bd/src/lib.rs:104`).

The claim is `bd update <id> --claim`: "Atomically claim the issue (sets assignee to you, status to in_progress; idempotent if already claimed by you)". On v1.1.2 code this is an atomic transition from open-and-unassigned-or-self to `in_progress` with `assignee = actor`, returning `ErrAlreadyClaimed` (exit 1, not 13) when another actor holds it. `bd ready --claim --json` claims the first ready match. The 1.2.1-only extras, `--if-assignee`/`--if-status` with exit 13 and `guard_mismatch: true` in `--json` failures, `bd unclaim --if-assignee`, and pools via `claim.pools`, do not exist on the pinned binary. One consequence Air had to work around: on 1.1.2 code `bd update -a X` writes the assignee with no check (`update.go:107` at tag v1.1.2), and a pencilled assignee on an open bead blocks every other worker's `--claim`, so a release has to reopen and unassign in one process (`reopen_unassigned`, `crates/bd/src/lib.rs:188-191`, air-0kk). (`bd update --help` local, 2026-08-17; tick 0300 rows 2.3 and 2.4, 2026-08-18.)

`bd blocked` lists blocked issues with their blockers; `bd dep cycles` audits cycles, which `bd dep add` also rejects at write time. Blocking edge types are `blocks`, `parent-child` (children blocked while the parent is blocked), `conditional-blocks` and `waits-for`; `related`, `tracks`, `discovered-from`, `caused-by`, `validates` and `supersedes` do not block. (https://github.com/steveyegge/beads/blob/main/docs/core-concepts/dependencies.md, accessed 2026-08-17.)

## bd 1.2.x facts

The rows from tick 0300 (2026-08-18) that Air's design still leans on. Sources for the table: the GitHub releases, tags and commits API for `gastownhall/beads`; `CHANGELOG.md` and `docs/RECOVERY-1.2.1.md` fetched raw; the local clone `~/projects/beads` at `d1e725d9f` with tag `v1.1.2` (the code released as v1.2.2); the installed binary. "Observed" means a command was run or a file read at a named commit; "documented" means an upstream doc or changelog says so and it was not exercised.

| Question | Answer on the pinned bd 1.2.2 | Source | Verified |
|---|---|---|---|
| Command surface present | `bd update --claim`; `bd ready --claim` and flags `--limit` (default 100), `--parent` (recursive), `--label`, `--label-any`, `--exclude-label`, `--assignee`, `--unassigned`, `--type`, `--exclude-type`, `--priority`, `--sort`, `--explain`, `--include-deferred`, `--mol`, `--gated`; `bd update --status/-a/--add-label/--remove-label/--append-notes/--acceptance/--design`; `bd gate`, `bd merge-slot`, `bd set-state`, `bd formula`, `bd mol`; global `--json`, `--actor`, `-C`, `--readonly`, `--sandbox`. | `git show v1.1.2:cmd/bd/{update,ready}.go`, `git ls-tree v1.1.2 cmd/bd/` | Observed |
| Command surface absent | `bd heartbeat`, `bd reclaim`, `bd unclaim`, `bd events`, `bd sync`, `bd serve`, `--if-assignee`, `--if-status`, `--brief`, `claim.pools`, `bd update --force`, and the refusal that stops `bd update -a X` overwriting another actor's live claim. `bd lease` and `bd claim` exist in no version. | same, plus `bd help <cmd>` locally | Observed |
| `bd ready` rule | `status = 'open'` only, plus the type, pinned, ephemeral, `is_blocked` and `defer_until` exclusions above; custom statuses excluded (upstream #5831); default cap 100. | `internal/workapi/ready.go`, `internal/storage/sqlbuild/ready.go` | Observed |
| Exit codes | `--claim` refused because another actor holds it: exit 1. Exit 13 exists only for the 1.2.1 `--if-*` guards and does not occur on 1.2.2. | `bd help update`; CHANGELOG 1.2.1 | Observed |
| Leases, where they live (1.2.1 and main only) | Not issue columns since migration 0055: a dolt-ignored `leases(issue_id, holder, granted_at, lease_expires_at, heartbeat_at, granted_node)` table that "lives only in the working set and is never part of committed history", materialised per clone. | `internal/storage/schema/migrations/ignored/0012_create_leases.up.sql`, `0016_*` | Observed |
| Lease TTL | `DefaultLeaseTTL = 5 * time.Minute`, a Go constant with no CLI flag and no config key; `bd reclaim --older-than` defaults to twice that. | `internal/storage/issueops/lease.go:19-42`, `cmd/bd/reclaim.go:227` | Observed |
| Who may heartbeat | Keyed on the actor string alone (`--actor`, `$BEADS_ACTOR`, or the git user): "any process presenting the same actor renews the lease." No session, PID or process identity is involved. | `lease.go:302-400`, `bd heartbeat --help` | Observed |
| Embedded Dolt concurrency | The only bd-level flock is `embeddeddolt/.lock`, taken during `bd init`. Ordinary commands open a short-lived connection and serialise on Dolt's own lock with exponential backoff (`MaxInterval 5s`, `MaxElapsedTime 0`); the CLI's Dolt open timeout is a fixed 30 s. Several worktrees writing through one `.beads` wait rather than being refused, and a burst can hit the 30 s timeout. | `internal/storage/embeddeddolt/{store.go:36-43,100-106, open.go:35-45}`, `cmd/bd/main.go:91` | Observed |
| Worktrees and `.beads` | Linked worktrees discover the repository's `.beads` through the git common dir with no redirect; `BEADS_DIR` overrides; `.beads/redirect` is for secondary full clones, one level only. `--readonly` blocks writes down to the store. | `docs/reference/worktrees.md`, `docs/reference/advanced.md:115-144` | Documented |
| Recovery from a 1.2.1-migrated store | In `.beads/embeddeddolt/<db>`: `dolt sql -q "DELETE FROM schema_migrations WHERE version > 53; …"`, then `DOLT_ADD` and `DOLT_COMMIT`; back up first; the cursor replicates, so recover every clone or recover one and push. Needs a `dolt` CLI. | `docs/RECOVERY-1.2.1.md` at tag v1.2.2 | Documented |

Tick 0300's recommendation, which is what Air did (§3 of the tick, 2026-08-18):

Pin bd at 1.2.2 and assert it at startup. It is the only tested line upstream stands behind, Homebrew installs it on any routine upgrade, and a stray 1.2.1 binary silently re-migrates the store. Air's `doctor` compares `bd --version` with `BD_PINNED` (`crates/cli/src/cmd/doctor.rs:13,37`). The cost of the pin is losing `bd unclaim` and the CAS guards, so Air's ledger is the CAS: `air claim` checks its own `claims` table before calling `bd update --claim`, and a release is `bd update <id> -s open -a ""` issued only by the ledger's owner.

Do not depend on bd leases, on any version. Even on main they are a fixed five-minute TTL, per-clone and ephemeral, enforced only on the node that granted them, renewable by any process presenting the same actor string, and reaped only when something runs `bd reclaim`. Air's ledger holds `(bead_id, actor, generation, lease_until, heartbeat_at, worktree)`; on bd the only durable signal is `status = in_progress` plus `assignee`, and Air treats those as mirrors of the ledger, re-asserted after a crash rather than inferred.

Use the minimal command surface, all verified present at tag v1.1.2: `bd ready --json`, `bd show <id> --json`, `bd list --status <s> --json` (custom statuses accepted here even though `ready` drops them), `bd update <id> --claim --actor <a>`, `bd update <id> -s <status> [-a <actor>|-a ""]` (unguarded on 1.2.2, so gate it behind the ledger), `bd comment`, `bd close --reason`, `bd dep list --json`. Not to be relied on: `heartbeat`, `reclaim`, `unclaim`, `events`, `sync`, `serve`, `--if-*`, `--brief`, `claim.pools`, `set-state`, `merge-slot`, `gate`, `formula`, `mol`.

## bd's dependency guard is two rules, not an ancestor walk

Kept whole from `docs/notes/2026-09-06-bd-refuses-the-ancestor-edge.md` (worker, 2026-09-06, working air-btz) because three places in the code cite it: `crates/bd/src/lib.rs`, `crates/cli/src/cmd/status.rs` and `crates/cli/src/cmd/selftest.rs`, and the `decomposition` skill tells filers about it.

air-btz's removal condition is "when the tracker itself refuses or reports an edge from a child to its own ancestor", so the first job was to find out whether bd already does. It does not, and the way it fails is more specific than either side had it. Nine routes were tried in a scratch `bd init` project on bd 1.2.2 (Homebrew), the pinned version; two of them plant the edge. An earlier version of the note said the condition was met and the check should not be built. That was wrong. It rested on five routes, all of which happen to be caught, and the adopter's coordinator answered that none of their sessions ever ran `bd dep add` at all; the beads came from a decomposition pass. That pointed at create time, which is where the hole is.

What refuses:

    $ bd dep add zz-v9t.1 zz-v9t          # child depends on its parent epic
    Error: cannot add dependency: zz-v9t.1 is already a child of zz-v9t. Children inherit
    dependency on parent completion via hierarchy. Adding an explicit dependency would
    create a deadlock

    $ bd dep add zz-v9t.1.1 zz-v9t        # grandchild -> the epic, dotted ids
    $ bd dep add zz-v9t.1.1 zz-v9t.1      # grandchild -> its parent task, dotted ids
    $ bd dep zz-v9t --blocks zz-v9t.1     # the other spelling of the first
    $ bd dep add zz-v9t.1 zz-v9t --no-cycle-check
    $ bd dep add --file edges.jsonl       # bulk wiring, whole-graph check
    $ bd create --graph plan.json         # child + direct parent edge in one plan
    …each refused, naming the deadlock or the duplicate parent-child edge.

    $ bd dep add zz-c08 zz-bdz            # two unrelated tasks: allowed
    $ bd update zz-c08 --parent zz-bdz    # make the blocker an ancestor afterwards
    Error adding parent dependency: dependency zz-c08 -> zz-bdz already exists with type
    "blocks" (requested "parent-child"); remove it first
    $ bd show zz-c08 --json | …           # parent: None; refused cleanly, no half-state

What does not:

    $ bd create "x" -t task --parent zz-v9t.1.1 --deps zz-v9t.1   # deps on the GRANDparent
    ✓ Created issue: zz-v9t.1.1.1                                  # no warning of any kind

    $ cat plan3.json     # G, M child of G, C child of M; one edge C --blocks--> G
    $ bd create --graph plan3.json
    Created 3 issues
      C -> zz-72f    G -> zz-3yf    M -> zz-mt3

In both, the bead ends up carrying a `blocks` edge on its own ancestor:

    $ bd dep list zz-72f --json      →  zz-mt3 parent-child ; zz-3yf blocks
    $ bd dep cycles                  →  ✓ No dependency cycles detected
    $ bd ready --json | …            →  zz-72f (grandchild) ready? False
                                        zz-3yf (grandparent) ready? True

The bead can never become ready: the ancestor cannot finish until its descendants do. Nothing reports it, and it renders as "not ready yet" like any queued bead. That is the incident.

The mechanism is the part worth keeping. bd's guard is not an ancestor walk. It is two independent rules. First, a `parent-child` edge row already exists between the pair, so any other type on the same pair is refused; this covers the direct parent, always, on every route above, which is why nothing aimed at a direct parent gets through. Second, a dotted-id prefix test: `zz-v9t.1.1` is visibly under `zz-v9t`, so an edge between them is caught however deep, while ids that do not encode the chain are not. So the hole is an ancestor two or more levels up whose id does not encode the chain. The decisive check that it is the id shape and not the depth is the same grandchild-to-grandparent shape, once with dotted ids and once without:

    $ bd dep add zz-v9t.1.1 zz-v9t    # dotted:   refused, "already a child of zz-v9t"
    $ bd dep add zz-72f zz-3yf        # unrelated: ✓ Added dependency … (blocks)

`bd create --graph` assigns flat ids and links by `parent_key`, so a wave filed from a plan file produces exactly the shape rule two cannot see. A decomposition pass is therefore the one ordinary way to build this deadlock, and it is silent.

This repo on 2026-09-06: 163 beads, 6 with a parent, 11 edges over them (`bd dep list air-80x.1 … air-80x.6 --json`). Every `blocks` edge was sibling-to-sibling and every ancestral edge was `parent-child`, which is bd's hierarchy and definitional. Zero instances, but zero is not the argument, because the shape is reachable and invisible when it happens.

Removal condition: delete this section, and the check it justifies (`WorkLedger::dep_list` and the ancestor check in `air status` and `air selftest`), when bd's guard becomes a real ancestor walk on the create path, testable by re-running the two accepted routes above and seeing them refused. Re-run it whenever the bd pin moves, in either direction: this is a measurement of one version.

## Gas Town: the Refinery, the verdict, and what Air copied

From `beads-and-gastown.md` §2.4 to §2.6 (2026-08-17).

The Refinery is Gas Town's per-rig merge queue. "When polecats complete work via `gt done`, the Refinery batches merge requests, runs verification gates, and merges to main using a Bors-style bisecting queue": rebase A..D as a stack, test the tip, bisect on failure. Gates (test, lint) are pluggable and batching is the core; an integration-branch path exists per epic, where "MRs from epic children merge to integration/<epic>" and land on main as one commit. A conflict creates a task for another polecat, and `bd merge-slot` serialises conflict resolution, one bead per rig with `status = in_progress` while held and `metadata.holder` and `metadata.waiters`, to prevent "monkey knife fights where multiple polecats race to resolve conflicts". Polecat completion is self-managed: "The Witness observes but does NOT gate completion." (https://github.com/steveyegge/gastown/blob/main/docs/design/architecture.md, https://github.com/steveyegge/gastown/blob/main/docs/concepts/polecat-lifecycle.md, `bd merge-slot --help` locally, accessed 2026-08-17.)

Heartbeats in Gas Town are agent heartbeats layered above beads, in three stores: the Deacon's `heartbeat.json` (5 min stale, 20 min very stale, then a poke), a per-session `gt heartbeat --state=working|idle|exiting|stuck` read by the Witness, and a `heartbeat:<EPOCH>` label on the agent bead "because `bd agent heartbeat` was never shipped." Its rule: "never declare an agent stuck from a single store. Cross-check tmux session activity." The supervision chain is a Go daemon, then Boot, Deacon, Witnesses and Refineries, all of them LLM agents past the daemon. (https://github.com/steveyegge/gastown/blob/main/docs/concepts/heartbeats.md and the README, accessed 2026-08-17.)

The cost and the criticism are on the record. Yegge: "You probably don't want to use it yet … It's also 100% vibe coded. I've never seen the code"; "Do not use Gas Town if you care about money" (Welcome to Gas Town, 2026-01-01). Tim Sehn of DoltHub measured a 60-minute session at about $100 and saw it merge a pull request despite failing integration tests (https://www.dolthub.com/blog/2026-01-15-a-day-in-gas-town/, 2026-01-15). Tenzin Wangdhen hit 141 orphaned Claude processes and poor observability, yet had 6 of 7 queued tasks land overnight (https://tenzinwangdhen.com/posts/gastown-good-bad-ugly/, 2026-02-19). Mark Atwood found it works for parity work with an external oracle and "collapses on novel design work", with "the verification chain remains open" (https://reviewcommit.substack.com/p/gas-town-a-review, 2026-05-14). All accessed 2026-08-17.

The verdict was and is: do not adopt Gas Town as the runtime; treat it as prior art. It is Go, so a Rust runtime would drive `gt` from a shell and inherit its process model; it hard-depends on tmux, Claude Code hooks, a Dolt server and `--dangerously-skip-permissions`; supervision, conflict resolution and triage are done by LLM agents, which is where the cost and the "chaotic and sloppy" behaviour come from, while Air's premise is that those jobs are done deterministically in code; and its own successor exists because it was too monolithic. Keep the beads data plane compatible so a Gas Town or Gas City user could point at the same `.beads`: their `gt:*` labels, `hooked` status, pinned beads, `bd merge-slot` and formulas are all plain beads features. (§2.6, 2026-08-17.)

What to copy from Gas Town, and what to skip: the three-layer worker model (identity, sandbox, session), redirect-to-shared-`.beads`, the batch-then-bisect merge queue with pluggable gates, the merge-slot mutex, wisps for patrol noise, the `heartbeat:<epoch>` cross-check against real process or tmux activity before declaring anything stuck, and a scheduler cap on concurrent workers for rate limits. Skip the LLM supervisors. Its "reliability is a dial, accept some lost work for throughput" principle is the one Air disagrees with: claim, heartbeat and close transitions are guarded, and nothing forces a claim except the reaper. (§5.3 items 4 and 5, 2026-08-18.)

## Recommendations that shaped the boundary

The rest of `beads-and-gastown.md` §5.3 (2026-08-18), with where each landed.

Integrate through a `bd --json` subprocess behind a Rust trait, so an HTTP client or a Rust store can replace it later. That is `WorkLedger` in `crates/bd/src/lib.rs:159-198` and its one implementation `BdCli`; `Issue` is `#[serde(default)]` so unknown fields pass.

Pin the bd version rather than feature-gating on it. Air took the stricter form: one pinned version checked by `doctor`, no probing for optional features, because everything Air needs exists on 1.2.2 and everything it would probe for is retracted.

Take the concurrency model as given. Embedded Dolt serialises writers, so N agents calling `bd` queue on the engine lock (an adopter ran four this way). Air runs no daemon and no server; it keeps bd out of every hook path instead, because `bd ready --json` costs 0.7 to 1.4 s against a 100 ms hook budget. The Stop hook reads `.air/ready.json`, a cache written by commands that already paid for a `bd ready` (`crates/cli/src/cmd/ready_cache.rs:1-4`, air-09i).

Keep beads plain: `metadata`, labels and `external_ref` rather than new fields. Air adds one custom status, `awaiting_review`, declared at `air init`, and nothing else.

## Alternatives to bd

From `bd-alternatives.md` (air-6hl, worker ledger, 2026-09-06), lightly trimmed. The owner that day: "I think we can do some research. But our project is currently integrated quite deeply." Every number below was re-derived on 2026-09-06 from this tree or a live API call; where the tree has moved since, the 2026-09-14 figure follows.

### The finding that reframes the question

`bd v1.3.0-rc.1` shipped on 2026-08-31 and restores everything Air worked around. Its release body (https://api.github.com/repos/steveyegge/beads/releases/tags/v1.3.0-rc.1, fetched 2026-09-06) lists work leases with `bd heartbeat`, `bd reclaim` and `bd unclaim`; compare-and-set via `bd update --if-assignee` / `--if-status`, "one atomic transaction, nothing written on a mismatch"; `bd serve`, "One process answering 41 OpenAPI-specified operations across 35 paths instead of a `bd` subprocess forked per call"; an events journal; Dolt as the only backend; and "about 28 migrations" applied in place on first invocation. The stable release is still v1.2.2 (releases API `/latest`, fetched 2026-09-06). So the two things that would most change Air's relationship to bd are the same upstream release, not a replacement: `bd serve` removes the per-process cost, and CAS removes Air's reason to own the claim check. Both are in an rc, and Air has been burned by an untested bd release once already.

### The exact bd surface Air uses

Air's whole dependence is `crates/bd/src/lib.rs`: one trait and one shell-out implementation. On 2026-09-06 the file was 397 lines and the trait had 10 methods. On 2026-09-14 it is 493 lines and 13 methods; `children`, `by_statuses` and `dep_list` were added by air-84u and air-btz. The table gives today's lines.

| Method | argv | Source (2026-09-14) |
|---|---|---|
| `ready` | `bd ready --json` | `crates/bd/src/lib.rs:387` |
| `in_progress`, `by_status` | `bd list --status <s> --json` | `lib.rs:395` |
| `children` | `bd list --parent <id> --all -n 0 --json` | `lib.rs:399` |
| `by_statuses` | `bd list --status <s1,s2,…> -n 0 --json` (one argument; a repeated `-s` silently overwrites on 1.2.2) | `lib.rs:218` (`by_statuses_argv`) |
| `dep_list` | `bd dep list <id> … --json` | `lib.rs:232` (`dep_list_argv`) |
| `show`, `show_all` | `bd show <id> … --json` (bd omits an unknown id and exits 0, so the caller compares) | `lib.rs:415,429` |
| `claim` | `bd update <id> --claim --actor <a>` | `lib.rs:436` |
| `set_status` | `bd update <id> -s <status>` | `lib.rs:441` |
| `reopen_unassigned` | `bd update <id> -s open -a ""` | `lib.rs:205` (`reopen_argv`) |
| `comment` | `bd comment <id> <text>` | `lib.rs:451` |
| `close_all` | `bd close <id> … --reason <r> [--actor <a>]` | `lib.rs:239` (`close_argv`) |

Seven distinct subcommands through the trait: `ready`, `list`, `show`, `dep list`, `update`, `comment`, `close`. Outside it, three calls that run once in a repo's life: `bd --version` for the pin check (`crates/cli/src/cmd/doctor.rs:27`), `bd init --prefix <p> --non-interactive --init-if-missing --skip-agents --skip-hooks` at `air init --write` (`crates/cli/src/cmd/init.rs:516-527`), and `bd config set status.custom awaiting_review` (`init.rs:534-538`).

Air reads twelve fields of a bead (`Issue`, `lib.rs:104`) against roughly sixty in `bd schema`, and writes four. What Air deliberately does not use is the load-bearing list, because a replacement need not provide it: no bd leases, heartbeats or CAS (Air's ledger owns both, `lib.rs:3-5`); no graph editing (`bd dep add`, `blocked`, `recompute-blocked`, `epic`, `graph`; Air reads the ready list and, since air-btz, one edge shape, and never writes an edge); no `bd create` (the coordinator files beads by hand with `bd create --validate`, and `Bash(bd create *)` is on the worker deny list at `crates/cli/src/cmd/launch.rs:40`); no `bd sync`, `serve`, `events`, `remember`, `prime` or bd git hooks (`--skip-agents --skip-hooks` at init keeps bd's agent prose and hooks out); no bd MCP server; and `Bead: <id>` trailers are Air's, read from git, not bd's (air-7kp).

What it costs, from `air audit` on this repo's ledger on 2026-09-06: bd median 1429 ms per process over 751,673 processes; `air status` p50 3902 ms, p90 5027 ms, max 26601 ms over 31,093 runs. The cost is per process, not per query (air-869, the median holding across `ready`, `list` and `show`), which is why `close_all` and `show_all` batch ids into one process. The store here was 50 MB against an adopter's 2.1 to 2.2 GB. Nothing bd is called from a hook.

### The properties a replacement must have

Derived from the surface above, each naming what breaks without it.

1. A dependency-aware ready list. `bd ready` is the one query Air cannot compute itself and the whole reason bd is here rather than a table. Without it the `ready:` line, the Stop nudge and `idle-without-claim` have no input.
2. An atomic claim, or a claim Air can guard. Air owns CAS, so the weaker form suffices: a claim write that is idempotent and reports failure.
3. JSON on every read, with tolerance for renamed fields costing one serde struct.
4. Custom statuses. `awaiting_review` survives only on beads that already carry it, but the gate still matches it.
5. Issue types: `epic` distinguishable from `task`, since air-f10 excludes epics from the claimable count and `air claim <epic>` is refused.
6. Validated description sections. `bd create --validate` refuses a bead with no `## Acceptance Criteria`; that is the coordinator's gate, and `air land`'s acceptance print reads those sections.
7. Single machine, no daemon, no network. Hooks answer in 100 ms; a tracker needing a running server is a new failure mode in the middle of the fleet. `bd serve` is a daemon, which is a real tension with this property and why 1.3.0 is not a free win.
8. A CLI, or a Rust library. A library would remove the per-process cost, the largest number above.

Two properties Air does not need: distributed sync (one machine) and graph editing (Air reads the graph, never writes it).

### Candidates

Stay on bd 1.2.2. Correct today. It satisfies every property except the second half of item 8, and the per-process cost is paid outside hooks. Its known defects are on file with workarounds built: the assignee lock on open beads (`reopen_unassigned`, air-0kk), `bd show --json` dropping comment text and `bd list --json` omitting closed beads (round note items 47 and 48), `bd ready` excluding custom statuses (#5831). Cost of staying: zero. Risk: bd is one author's 225k-line Go project whose last stable release is a retraction of the one before it.

bd 1.3.0 when it leaves rc. Watch, do not adopt while it is an rc. It gives CAS and a single serving process (items 2 and 8). Against it: 28 in-place schema migrations on first run, a daemon in tension with item 7, and the 1.2.1 precedent. The re-read trigger is the stable tag.

`Dicklesworthstone/beads_rust` (`br`). Rust; 1,082 stars, v0.5.10, last pushed 2026-09-06 (GitHub API, up from 1,052 stars and v0.3.2 on 2026-08-15, so maintained rather than frozen). Its README gives `br ready` returning "only unblocked, actionable issues", `br update --claim` with exclusive locking, custom statuses via `policy.yaml`, `--json` on every command, `br lint`, full `dep` commands; SQLite, no daemon. That is items 1 through 7. Still not worth a swap, for three reasons from its own README: it is store-incompatible with bd ("Its length-prefixed content hashes deliberately differ from classic bd hashes. Use JSONL interchange"), so every store is a one-way export and import; it is a binary with no library crate, so Air would still pay per process, which removes most of the reason to move; and it is a fork of an architecture upstream abandoned ("is being replaced with approaches better suited to Steve's vision"), a bet against upstream by a repo with no measured pain from upstream. (https://github.com/Dicklesworthstone/beads_rust, fetched 2026-09-06.)

`delightful-ai/beads-rs`. 24 stars, 38 open issues, last pushed 2026-08-19; a git-refs redesign, alpha. No. (GitHub API, 2026-09-06.)

A plain SQLite table of Air's own. No, and this is the one worth arguing. Air already runs a SQLite ledger and an `issues` table is mechanically easy. What Air would be building is item 1, a dependency graph with a correct ready-list query, cycle detection and the epic and child semantics air-f10 depends on, plus item 6's validation and a CLI a person can drive by hand. That is the whole of a work tracker, and the `do-less` question is what recorded failure it removes. There is none: the ledger has no row saying bd cost this repo a bead, an hour or a wrong answer. It would save 1.4 s per process, paid in no hook, and would make every bd habit Air left alone into Air's problem. This is the Gas Town shape: a framework growing toward completeness rather than toward the owner's projects getting built. If bd became unavailable rather than slow, this is the fallback.

### What a swap costs, behind `WorkLedger`

A new backend writes a second `impl WorkLedger` (estimated 150 to 250 lines by analogy with `BdCli`), its own `Issue` deserialisation if the JSON differs (0 to 40 lines), and the ready-list query if the backend does not compute it, which is the whole of the SQLite candidate above rather than a line estimate. In `crates/cli`, five files name the concrete type `BdCli` (on 2026-09-14: `cmd/claim.rs` 6 mentions, `cmd/status.rs` 4, `cmd/selftest.rs` 2, `cmd/ready_cache.rs` 1, `cmd/budgets.rs` 1 inside a catalogue string); the other five of the ten files that use `air_bd` touch only `Issue` and `WorkLedger` and would not change. The honest estimate: one new file and about 15 edited lines to make `bd_for` return the new type. Call it a day, if and only if the replacement supplies the ready list. Outside the trait: the three one-shot `air init` and `doctor` calls, about 30 lines, and `BD_PINNED` with its check and surface notice.

### Verdict

Stay on bd. Change nothing. Watch one thing.

The integration is deep in the sense the owner meant, seven subcommands, thirteen trait methods, 751k recorded processes, but it is deep through one file and one trait, and that is the shape that makes it cheap to leave. The measured cost of staying is 1.4 s per process, paid outside every hook path, in a repo whose ledger records no failure caused by bd.

The one thing to watch is bd v1.3.0 leaving rc. When it does, the questions to re-ask are whether `bd serve` removes enough of the per-process cost to be worth a daemon inside the fleet, and whether upstream CAS lets Air's `claims` table stop being the authority on who holds a bead. Neither is answerable against an rc.

Removal condition for this section: when bd 1.3.0 is stable and those two questions are answered, or when the ledger records a first failure attributable to bd. Either way it is re-derived, not amended: the numbers in it moved once in three weeks and again in the week after.

## Sources

Every URL and path is cited inline where it is used. The local material behind them, all read-only: `bd --version` (2026-09-14: `bd version 1.2.2 (Homebrew)`); `bd help`, `bd schema` and `bd <cmd> --help` on the installed binary (2026-08-17/18); the local clone `~/projects/beads` at `d1e725d9f` with tag `v1.1.2` (2026-08-18); a scratch `bd init` project on 1.2.2 (2026-09-06); `air audit` on this repo (2026-09-06); `crates/bd/src/lib.rs` and `crates/cli/src/cmd/` at the lines cited (2026-09-14). Upstream paths under `docs/` without a URL are files in the `gastownhall/beads` repository at the commit or tag named beside them.
