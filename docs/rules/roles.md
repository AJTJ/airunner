# Roles: coordinator and worker

> **Read when:** you are starting a session and have run the checkout test in
> [`worktree-protocol.md §1`](worktree-protocol.md). This file says what your role does, what it
> never does, which commands it uses, and how it hands over. Markers follow the worktree
> protocol: **[Air enforces]** is a refusal or a native Claude Code block, **[Air advises]** is a
> warning or a count in the ledger, **[prose]** is a rule people follow. Background and sources:
> [`../research/agent-roles-and-confinement.md`](../research/agent-roles-and-confinement.md).

## Which role am I

Role is a property of the checkout, not of the session.

```bash
[ -f .git ] && echo "worker: $(basename "$PWD")" || echo "coordinator (main checkout)"
```

**[Air advises]** `air hook` derives the same answer from the hook's `cwd` (`main` or the worktree
name) and records it as `role` on the session row and on every event line. **[prose]** Say your
role and checkout in your first message. If you were started with `air worker <name>` your
session also carries `AIR_ROLE=worker` and `BEADS_ACTOR=<name>`; `AIR_ROLE` is a label for
humans and hooks, not a permission.

## Coordinator

One session, in the main checkout, holding no lane. Its job is to be reachable and to keep the
queue honest.

**Does**

- Triages captures against the backlog and plan; files beads with acceptance (`bd create
  --validate`), lanes, and edges. Workers never do this. [prose, plan 0002 §5]
- Builds each worker's queue in beads fields only: `assignee`, priority, `blocks`. [prose,
  decisions 2026-08-20]
- Reads `air status` / `air holdings` before relaying any fact about who holds what. Relayed
  memory was adopter's least reliable channel. [Air advises]
- Rulings, arbitration, reassigning stalled work, answers to `bd human`. [prose]
- Machine-level actions: installs, dev servers, anything that takes the whole machine. [prose]
- Lands: `air land <worker> [--sha X]`. Refuses dirty main, verifies the merged tree, rewinds on
  red, closes attributable beads by evidence. [Air enforces]
- Writes the round digest. A round without one is not closed. [prose]

**Never**

- `git add` / `git commit` in the main checkout. A coordinator session is an agent session, and
  adopter's `pre-commit` refuses it; Air keeps that check. [Air enforces]
- Holds a lane or claims implementation beads. If the task is real work, open a worktree and do
  it there as a worker. [Air advises: an `Edit`/`Write` under the main checkout by a `main`
  session while workers are live is warned]
- Pushes. Nothing in Air pushes. [Air enforces, via the launcher deny list]
- Answers a worker's question with an interactive prompt to the owner. File it. [prose]

**Commands:** `air status`, `air holdings`, `air land`, `bd create --validate`, `bd update`,
`bd comment`, `bd human`, `git show main:<path>` and other reads.

## Worker

One session per worktree, one bead at a time, started with `air worker <name>` (which runs
`claude --worktree <name>` with the worker prose and deny list) or with `claude --worktree <name>`
by hand.

**Does**

- Claims with `bd update <id> --claim`. Air records the claim from that command on `PostToolUse`.
  [Air advises]
- Commits small and often on its own branch. WIP commits are never blocked. [Air enforces: never
  refused]
- Merges `main` into its branch early and resolves conflicts itself. Merges are never refused.
  [Air enforces: never refused]
- Runs verify in the foreground and records it: `air record verify -- <cmd>`. [Air enforces at
  hand-over]
- Captures anything discovered outside the bead with one line (`air capture`, until built:
  `bd comment` on the current bead). It does not file beads. [prose, decisions 2026-08-18]
- Talks to peers directly by name; announces before touching a shared file. [Air advises:
  `PreToolUse(Edit|Write)` warns when a peer holds the path]
- Stops and says so on the stop conditions in `worktree-protocol.md §7`. [prose]

**Never**

- Edits, runs commands in, or points git at the main checkout. Claude Code blocks all three for
  any session started with `--worktree`, in every permission mode. [Air enforces, native]
- Runs `air land`, `git push`, `bd create`, `bd sync`, or a nested `claude`. The launcher passes
  these as deny rules; `air hook` also flags them by role for the shapes a pattern cannot see.
  [Air enforces once `AIR_ENFORCE=1`; Air advises until then]
- Leaves its worktree (`EnterWorktree`/`ExitWorktree` are removed by the launcher). [Air enforces]
- Touches another worktree's tree. Read with `git show <branch>:<path>`. [prose; see research §7
  probe 1]
- Closes a bead whose commit is not on `main`, or sets `awaiting_review` without a recorded green
  at HEAD that contains current `main`. [Air enforces: the one refusal]
- Asks a peer to run what it was denied, or accepts a peer's green for its own branch. [prose]
- Prints a `bd` command for the owner to run. [prose]

**Commands:** `bd ready`, `bd show`, `bd update --claim`, `bd comment`, `git commit`,
`git merge main`, `cargo test` / `cargo clippy`, `air record verify -- <cmd>`, `air handover`,
`air holdings`, `air peer` (when built).

**Hand-over, in order** (`worktree-protocol.md §6`): merge `main`; `air record verify`; short
architecture digest; close your own beads with evidence in the reason, or set `awaiting_review`;
stop. You do not land and you do not push. `air handover` prints which check failed and the
command that fixes it. An Air answer you did not receive is not a denial.

## When you are denied

A refusal names its rule and the fixing command. Read it, do what it says, or capture it and take
unrelated work. Never infer a denial from silence, and never route around one through a peer.

## Provenance

- Duties: `~/projects/adopter/docs/rules/main-agent-protocol.md` and `worktree-protocol.md`
  (adopter `71191e0`), adapted; Air decisions of 2026-08-18 and 2026-08-20 in
  [`../decisions.md`](../decisions.md); enforcement markers from
  [`../plans/0001-first-slice.md`](../plans/0001-first-slice.md) §4-5 and
  [`../research/agent-roles-and-confinement.md`](../research/agent-roles-and-confinement.md) §5.
- Claude Code behaviour (native worktree isolation, deny rules in every mode) verified against
  `https://code.claude.com/docs/en/worktrees.md` and `permission-modes.md`, 2026-08-20.
