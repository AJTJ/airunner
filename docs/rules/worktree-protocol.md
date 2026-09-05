# The worktree protocol

> **Read when:** you are an agent and have not yet established which checkout you are in. That is
> the first question of any session, because every other permission depends on the answer. This
> is the protocol shape adopter runs today (adapted from
> `~/projects/adopter/docs/rules/worktree-protocol.md` @ `f2ca891`); the parts Air is meant to
> turn from prose into machinery are marked **[Air enforces]** or **[Air advises]** with the
> section of [`../plans/0001-first-slice.md`](../plans/0001-first-slice.md) that says so.
> Everything unmarked stays a rule people follow.

## 1. Establish where you are, mechanically, before anything else

Do not infer it from the path, and do not assume. **`.git` is a directory in the main checkout and
a file in a worktree.** That single fact is the test:

```bash
[ -f .git ] && echo "worktree: $(basename "$PWD")" || echo "main checkout"
```

Corroborating signal, also cheap: `git rev-parse --git-dir` differs from `--git-common-dir` in a
worktree. Air keys its ledger off the same fact — `.air/ledger.db` lives at
`git rev-parse --git-common-dir`, shared by every worktree (plan 0001 §2).

**Say which you are in your first message.** Two adopter agents were confused with each other on
2026-08-13, and one was credited with the other's finding.

**Check your actor identity rather than assuming it, because your bead claims are only
attributable while it is set.** `bd update --claim` writes the actor into the assignee; with the
actor unset that is the git identity — the same for every session on this machine — or nothing at
all. A landing keyed on the assignee cannot close an unnamed claim.
**[Air enforces]** `air claim` wraps `bd update --claim` with the actor and CAS, and mirrors the
claim in the ledger's `claims` table (plan 0001 §2 row 3, §3). A claim made through `air claim`
cannot be anonymous.

**Claim one bead at a time.** A queue you are draining is not a set you claim.
**[Air advises]** `air holdings` / `air status` print who holds what per worktree, so a
double-claim is visible to the coordinator without reading `bd list` by hand (plan 0001 §3).

## 2. What follows from the answer

| | Main checkout | Worktree |
|---|---|---|
| `git add` / `git commit` | **Never.** The owner's history | **Yes**, and often — small commits as you finish each piece |
| `git push` | Never | Only when the owner's process says so |
| `git merge main` into your branch | n/a | **Yes**, and early. Staleness compounds |
| Machine-level actions (installs, native builds) | Owner / coordinator only | Not yours |
| `cargo test`, `cargo clippy`, verify | Free | Free — no lease |

**The main checkout is not yours.** Read it freely — `git show main:path`, `git log main`,
reading files — and never write to it.

**[Air enforces]** WIP checkpoint commits on your own branch are **never** blocked and merges are
**never** refused (plan 0001 §4). Air's only refusal is at hand-over (§6 below).

**[Air advises]** `PreToolUse(Edit|Write)`: if the file you are about to edit is held by a peer
(uncommitted, or committed since your merge-base) Air warns with the peer and sha — it never
denies (plan 0001 §5). `PostToolUse(Edit|Write)` journals the intent to touch at zero token cost.

## 3. Compiling is a shared resource

**Cargo takes the whole machine.** `build.jobs` defaults to every logical core *per invocation*,
so two agents running `cargo check` are each asking for all of it. the adopter measured a 12–16 s
`cargo check` at **219.7 s** under contention, with load average peaking at **207** on 16 cores.

**Worktrees do not isolate the build.** They isolate the branch, not the CPU. Two settings, per
worktree, nothing to take or release:

| Setting | Value | Fixes |
|---|---|---|
| `CARGO_TARGET_DIR` | this worktree's own `target/`, absolute | the target-dir lock, across worktrees |
| `CARGO_BUILD_JOBS` | `hw.logicalcpu / <number of builders>`, derived at setup | the CPU |

Derive from `sysctl -n hw.logicalcpu`, never pin: a replaced machine must not inherit an old
laptop's cap. A long-lived watcher (`cargo watch`) inside one worktree needs its own target dir,
or every `cargo check` beside it blocks on `Blocking waiting for file lock on build directory`.

**There is no pool.** adopter's owner decided on 2026-08-14 to isolate rather than coordinate —
no supervisor, no jobserver, nothing to leak or wedge. If a pool is ever built, the acquisition
order is part of building it: take any lease *before* a build slot, never hold two slots. Air does
not build a pool (plan 0001 §9, non-goals).

**Batch what you build, and never wait to compile.** Do the whole change, then compile once. Almost
every task has a compile-shaped part and a larger part that is not: reading, writing the test or
the doc, anything in `bd`, another bead entirely.

**What this rule is NOT.** It is not "agents do not compile". Rust that has not been compiled is
usually simply broken. **A recorded green is the only green** — see §6.

**Read the load average.** `uptime` is free. Above 2× logical cores: stop compiling and say so
(metastable-failure territory; adopter hit 207). Under 2, absolute, or a timing you take means
nothing — label it an upper bound. Before you diagnose a flaky port, a timeout or a failing test,
check the load first; contention mistaken for a defect is the feedback loop.

## 4. Never infer that you are blocked — provoke the guard and read what it says

A guard denies you out loud, with a message naming its rule. Silence means you were never
blocked. A adopter agent reasoned that its tests must need a lease, never ran them, and asked a
peer to run them instead; nothing had denied it, and it cost every verification on that branch.
**[Air enforces]** every Air refusal or warning prints exactly which check failed and the command
that fixes it (plan 0001 §1, §4). An Air answer you did not receive is not a denial.

## 5. Talk to your peers directly

List them, message them by name, without routing through the owner. Announce what you are touching
before you start on anything shared. Ask rather than wait. Tell peers when you merge something they
depend on. SendMessage stays the channel; Air never sends messages (plan 0001 §6).
**[Air answers]** `air holdings` says who is in which file and `air status` each worker's head
and whether it is green, from the ledger and git, so the announcement is a fact, not a memory
(plan 0001 §3). `air peer` and `air merge-advice` were planned there and never built; the
overlap hook named the first one for a week before anyone ran it (air-w91).

**Never ask a peer to run what you were denied**, and never accept a peer's green for your branch —
they ran their tree, not yours.

## 6. Handing over

In order, and none of it is optional:

1. `git merge main` and resolve conflicts yourself. Conflict resolution belongs to whoever has the
   context, not to the owner reading a diff an hour later. Generated files (`.beads/issues.jsonl`)
   are regenerated by their merge driver, never hand-merged.
2. Run the verify command in the foreground and **record** it: `air record verify -- make verify`
   (`make verify` is this repo's one gate: fmt, clippy, tests, `air selftest` on the tree's own
   build, cheap first; decisions 2026-08-22). Never declare green off a piped or backgrounded exit code, and never off model text.
3. A short architecture digest of the shape, not a changelog: new tables, new commands, anything
   touching the spine, and **any decision that could reasonably have gone the other way**.
4. Close your own beads, with evidence in the reason. **Never print a `bd` command for the owner to
   run.**
5. Hand over for landing: `bd update <id> -s awaiting_review`. The coordinator lands it with
   `air land <id>` (or `air land --all`). You do not land, and you do not push; `air land` is on
   the worker deny list in every permission mode.

**[Air enforces]** This is the one refusal. `air handover` (and the `Stop` hook, advisory for one
round first, then blocking) refuses `awaiting_review`/close unless: a `verify_runs` row exists for
this worktree at HEAD with exit 0; HEAD contains current `main`; fitness/docs-check are green at
HEAD; the bead is claimed by this actor (CAS). It prints which check failed and the fixing command
(plan 0001 §4, §5). `air land` is the coordinator's port of `land.sh` (built 2026-08-22, air-3pz):
refuse a worker, a worktree, a branch other than main, a branch that does not contain main, or a
recorded green that is not at the branch head. Since air-odv (2026-08-29) it then builds the
landing commit off main with `git commit-tree` and fast-forwards main onto it: **main is never
moved to a commit that has not been verified**, so there is no rewind, no `git reset --hard`, and
no second verify — the branch contains main, so the landing commit's tree is the one the worker's
green already describes. The dirty-tree refusal went with the reset that was its only reason.
Every attempt is a `landings` row, refusals included.

## 7. Stop, and say so

Escalating is a success condition. Stop on three attempts at the same wall, the same error twice,
anything irreversible, work outside your bead, acceptance you cannot reach, or a guard denying you.
Say what you tried, what happened, and what you think is needed — then take unrelated work.

**To whom.** In order: the **coordinating session** if one is running — it holds no lane and its
job is to be reachable. Otherwise **`bd human <id>`**, which is durable and survives your session
ending. **Never an interactive prompt to the owner:** it leaves your tool loop and nothing rescues
you until a human arrives (that cost adopter 4.5 hours on 2026-08-15). Escalate by filing, never
by prompting. **[Air enforces]** no hook ever blocks on a question (plan 0001 §4).

**Record each failed attempt as `bd comment <id>`**, not as a mental tally — append-only and
timestamped, so "three attempts" is visible to a peer, the coordinator and the owner instead of
counted by the same degraded context that produced the failures.
**[Air advises]** repeat claims on one bead and the number of beads one session picked up and put
back are counters Air can derive from `claims` and `sessions` (plan 0001 §2, §3); the count is the
signal, the release is still the right move.

## Provenance

- Source: `~/projects/adopter/docs/rules/worktree-protocol.md` (adopter `f2ca891`),
  adapted 2026-08-18.
- Kept: the mechanical `.git` file-vs-dir test, the main-vs-worktree permission table, the
  compile-contention section (measurements attributed to adopter), the guard-provocation rule,
  the peer rules, the hand-over order, the stop-and-escalate rules with the OTP-style counters.
- Stripped: `DB_SUFFIX`/`adopter_<worktree>` per-worktree Postgres DBs, `make worktree-setup` /
  `worktree-env` / `fleet` / `queue` / `land` / `verify` / `docs-check` targets, `lease-guard.sh`
  `NEEDS` list, the `runtime` lease, native/Expo builds, links to adopter plans 0010/0015 and
  notes, bead ids (`fd-*`).
- Added: `[Air enforces]` / `[Air advises]` markers citing `docs/plans/0001-first-slice.md`.
