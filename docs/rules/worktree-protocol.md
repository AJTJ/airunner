# The worktree protocol

> **Read when:** you are an agent and have not yet established which checkout you are in. That
> is the first question of any session, because every other permission depends on the answer.
> `[Air enforces]` marks a refusal; `[Air answers]` marks a fact a command prints. Everything
> unmarked is a rule people follow.

## 1. Establish where you are before anything else

Your role is `AIR_ROLE`, which the launcher sets; your name is `BEADS_ACTOR`. The directory
does not decide what you may do. To know which checkout you are standing in, `.git` is a
directory in the main checkout and a file in a worktree:

```bash
echo "$AIR_ROLE $BEADS_ACTOR"; [ -f .git ] && echo "worktree: $(basename "$PWD")" || echo "main checkout"
```

Say who you are in your first message: two agents were confused with each other on 2026-08-13
and one was credited with the other's finding.

Claim through `air claim`, never `bd update --claim` by hand. `[Air enforces]` the claim
carries your actor and is mirrored in the ledger, so it cannot be anonymous. Claim one bead at
a time. `[Air answers]` `air status` and `air holdings` show who holds what.

## 2. What follows from the answer

| | Main checkout | Worktree |
|---|---|---|
| `git commit` | allowed since 2026-08-29; push is the line, not the commit. Under the design in `docs/plans/0009-fleet-system-design.md` nobody works here at all | yes, and often: small commits as you finish each piece |
| `git push` | never | never; the owner pushes |
| `git merge main` into your branch | n/a | yes, and early; staleness compounds |
| machine-level actions (installs, native builds) | owner or coordinator | not yours |
| `cargo test`, `cargo clippy`, `make verify` | free | free; no lease needed |

The main checkout is not yours to write. Read it freely (`git show main:path`, `git log
main`). `[Air enforces]` an Edit or Write whose path leaves your worktree is denied. WIP
commits on your own branch are never blocked and merges are never refused; Air's only
refusal is at hand-over (§5). `[Air answers]` before an edit, the hook tells you when a peer
is already in that file.

## 3. Compiling is a shared resource

Cargo takes the whole machine: `build.jobs` defaults to every logical core per invocation, so
two `cargo check`s each ask for all of it. An adopter measured a 15 s `cargo check` at 220 s
under contention with the load average at 207 on 16 cores. Worktrees isolate the branch, not
the CPU. Give each worktree its own `CARGO_TARGET_DIR` and a `CARGO_BUILD_JOBS` derived from
`sysctl -n hw.logicalcpu` divided by the number of builders. Batch what you build and compile
once; read `uptime` before diagnosing a slow test, and above twice the core count stop
compiling and say so. There is no build pool by decision (2026-08-14).

## 4. Never infer that you are blocked

A guard denies you out loud with the rule it applied. Silence means you were never blocked.
`[Air enforces]` every Air refusal names the check that failed and the command that fixes it.
Never ask a peer to run what you were denied, and never accept a peer's green for your branch.

## 5. Handing over

The sequence is this repo's, in `CLAUDE.md` ("This repo's work flow"): merge `main`, commit
the digest with its front matter, record the verify (or wait for the lane's), then close your
own bead with proof. `[Air enforces]` the close is refused without a recorded green at a commit
that contains `main` and every commit carrying the bead's trailer, a claim, and a tracked
digest; the refusal names what is missing. Landing is `air land`, denied to workers in every
permission mode; `main` only moves by fast-forward onto a verified tree.

## 6. Stop, and say so

Stop on three attempts at the same wall, the same error twice, anything irreversible, work
outside your bead, acceptance you cannot reach, or a guard denying you. Say what you tried and
what you think is needed, then take unrelated work. Escalate with `air capture "<text>"` or a
message to the coordinator; never with an interactive prompt to the owner, which leaves your
tool loop with nothing to rescue you (4.5 hours lost that way at an adopter, 2026-08-15).
`[Air enforces]` no hook blocks on a question and `AskUserQuestion` is denied to workers.
Record each failed attempt as `bd comment <id>` so the count is visible to others.

## Provenance

Adapted 2026-08-18 from an adopter's worktree protocol (their `docs/rules/worktree-protocol.md`
at `f2ca891`). Condensed 2026-09-14 from about 1,900 words to this, dropping the sections that
had gone stale (commits in main, `awaiting_review` hand-over, a `bd human` command that does
not exist) and the plan 0001 citations, which now point at `docs/design.md`.
