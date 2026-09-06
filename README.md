<p align="center">
  <img src="assets/logo.svg" alt="AIRunner" width="640">
</p>

Multi-agent accountability and verification system. Lighter than air.

Several AI coding agents work one repository at the same time, each in its own git worktree.
Air is one Rust binary that holds the facts they would otherwise carry in chat: who is editing
which file, which commit actually passed verification, who holds the shared port. It works
beside [beads](https://github.com/steveyegge/beads) for issues and Claude Code for the agents,
and replaces neither.

## What it records

Every fact below is written when it happens, not reconstructed afterwards from a transcript.

- **Verification runs**, keyed to the commit they ran at, with the command, the duration, and
  whether the tree was dirty. A green belongs to a commit, not to a person's memory of one.
- **Who is in which file**, from the editor's own tool calls, across every worktree at once.
- **Claims and leases**: which agent holds which task, and which holds the simulator, the port,
  the browser.
- **Every decision Air made**, with its reason and its denominator, as append-only JSON. What
  Air did not check is distinguishable from what it checked and allowed.
- **What its own machinery costs**: how long each timing budget waited and whether it ran out,
  how much each `bd` call cost, how many messages agents sent each other.

## What it refuses

One thing. **An agent cannot close a task without a recorded pass at a commit that contains
`main`.** Not a claim of one, not a description of the approach: a recorded run at that exact
commit. The refusal names the command that fixes it.

Everything else Air says is a fact or an answer. It does not review code, choose work, or tell
an agent how to do its job.

## A human is always in the loop

Every agent Air launches is an interactive terminal you can open, watch, and type into. Air's
launchers never start headless sessions, and nothing Air does may hide what an agent is doing.
`air status` is one screen of live state: sessions, claims, what is green, who overlaps whom.

## Install

You need `git`, a Rust toolchain, [beads](https://github.com/steveyegge/beads), and Claude Code.

```sh
cargo install --path crates/cli   # from a checkout of this repository
cd /path/to/your/repo
air init --prefix <your-beads-prefix> --write   # dry run without --write
air doctor                                      # should exit 0
```

`air init` writes the Claude Code hooks, the MCP server entry, the role prose, and a
`.claude/air.json` you own. It prints everything it would do before it does any of it.

Adopting Air into a repository that already has its own process:
[`docs/rules/adopting-air.md`](docs/rules/adopting-air.md).

## Day to day

Start the sessions:

```sh
air coordinator                          # the main checkout, with Air's channel attached
air worker w1 --task "<a complete task>" # its own worktree, its own terminal
```

A worker takes a task, does it, and closes it with proof:

```sh
air claim <id>
# ... the work, and a digest committed with it
git merge main
air record verify -- <your verification command>
bd close <id> --reason "<the proof>"
```

The `air record verify` line is what the one refusal reads. Run it last, after the merge, so
the green belongs to the commit that is actually being handed on.

The coordinator watches `air status`, triages what workers capture (`air capture "<one line>"`)
into tasks, and lands branches. Air's channel raises a condition when something needs a person:
an idle agent holding a task, a hand-over without a green, a branch ready to land, a lease held
by a session that died.

## What your repo provides

Air records and refuses; four things have to come from the repo, and `air init` prints what is
missing rather than guessing.

- **A verify command.** Anything that exits non-zero on red. Air's one refusal reads a green
  recorded at a commit, so a repo with no such command has nothing to record.
- **One paragraph on how a finished task is handed on.** `.air/roles.md` deliberately does not
  say: some repos hand over for review, some close with proof. That choice is the repo's.
- **A `Bead: <id>` trailer on the commits that do a task's work.** Attribution reads the
  trailer and nothing else; a commit without one is attributed to nothing.
- **A `.worktreeinclude`**, if a build needs files git ignores (keys, `.env`). Air fills each
  worker's worktree from it.

Everything else Air leaves to you on purpose: how the verify is scoped, how code is reviewed,
which commands are too dangerous for an agent, whether landing is `air land` or your own script.
The first adopter keeps several thousand lines of that around Air, and none of it is Air's to
own. `docs/notes/2026-09-06-what-a-repo-provides.md` is the audit, item by item.

## Your project and Air

Your project decides what verification means, what to build, how code is reviewed, and how
changes reach `main`. Air records the results and answers who is waiting on whom.

The line is deliberate. Air removes friction (relayed facts, drift, collisions) and does not
direct the work, because a capable model does not need directing. Every rule it ships names the
recorded failure it prevents **and** the condition under which it is removed, and `air audit`
prints both against the ledger so a rule that has stopped earning its place shows up as a fact
rather than as somebody's hunch.

## Status

Early, and honest about it. In use on two repositories, this one included. The surface moves
between releases and `air install` tells an already-installed repository what changed.

- Decisions, dated: [`docs/decisions.md`](docs/decisions.md)
- What is next: [`docs/plans/0005-roadmap.md`](docs/plans/0005-roadmap.md)
- What agents are told: [`docs/rules/roles.md`](docs/rules/roles.md)
- The evidence behind the design: [`docs/research/`](docs/research/)

## Layout

`crates/ledger` (SQLite plus the event log), `crates/hooks` (Claude Code hook logic, pure and
fast), `crates/bd` (the beads boundary), `crates/cli` (the `air` binary).

## Licence

Apache-2.0. See [LICENSE](LICENSE).
