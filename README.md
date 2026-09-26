<p align="center">
  <img src="assets/logo.svg" alt="AIRunner" width="640">
</p>

Multi-agent accountability and verification system. Lighter than air.

## Philosophy

Do less. Most agentic systems try to do too much. The model knows how to do the work and keeps
getting better at it, so Air does not plan the work or tell agents how to do it. What it does
is practical: a merge queue, a few checks, and help keeping a fleet of sessions running.

## What Air is

- **A merge queue.** Workers finish branches, and a verification lane merges them into one batch
  and lands it on `main`.
  - Your project defines the check: one command that exits 0 when the project is good, usually
    your tests. For example `make verify`.
  - The lane runs it once per batch, and Air records which commit passed.
  - A bead closes only on a passing run.
- **Sessions.** Air starts the coordinator, the workers and the lane, each in its own git
  worktree and tmux session.
- **Shared resources.** Leases for anything one agent can use at a time, such as a port or a
  simulator.
- **Keeping the fleet moving.**
  - The coordinator is told when a worker goes idle, silent or away while holding a bead, when a
    branch is ready to land, and when a lease is held by a session that has died.
  - A worker with nothing claimed is offered the beads that are ready.
  - Two workers editing the same file are warned.
  - `air status` shows when a session stopped, for example at an account limit, and what else is
    running in each worktree.
  - Sessions set a recurring wake so they pick up again after a pause.

Air supports Claude Code only, for now.

## How you use it

You talk to the coordinator. It writes the beads, the workers do them, and you check their results.
Every session is a full harness running in a worktree, in tmux that, you can interact with.

## The pieces

Air is one Rust binary, `air`, plus the files it keeps in `.air/`.

- **Launchers.** `air coordinator`, `air worker` and `air lane` start sessions in their worktrees.
- **Ledger.** One SQLite file with claims, verification runs, leases and landings, plus a daily
  event log.
- **Hook.** `air hook` runs on every tool call. It keeps a worker's edits in its worktree and
  refuses a close that has no passing run.
- **Channel.** `air mcp` tells the coordinator when something needs attention.
- **Merge queue.** `air record`, `air batch cut` and `air land`.
- **Diagnostics.**
  - `air status` shows the whole fleet on one screen.
  - `air audit` shows how often each rule fired, so rules that never fire can be removed.
  - `air doctor` checks the install and the pinned beads version.
  - `air selftest` proves every check can fail and pass.

## Upcoming

- Support for [Pi](https://github.com/earendil-works/pi) and other open-source harnesses.
- Messaging between agents, managed by Air.
- Fleets spread across several machines, with every agent still working through Air.

## How to install it

Needs git, Rust, tmux, Claude Code and [beads](https://github.com/gastownhall/beads) (`bd` 1.2.2).

```sh
cargo install --path crates/cli     # from a checkout of this repository
cd /path/to/your/repo
air init --write
```

Make `make verify` run your check, commit, and run `air coordinator`. If agents share a port or a
device, list the commands that use it under `leases` in `.claude/air.json`.

[examples/minimal](examples/minimal) is a three-file project after `air init`. It shows a check
and every file Air adds.

## Inspired by

- [Gas Town](https://github.com/gastownhall/gastown): the batch-then-bisect merge queue, and
  checking a heartbeat against the real process.
- [beads](https://github.com/gastownhall/beads): the task store Air works beside.
- [Symphony](https://github.com/openai/symphony): isolated runs per piece of work.
- [bors-ng](https://github.com/bors-ng/bors-ng), [homu](https://github.com/rust-lang/homu) and
  [Zuul](https://opendev.org/zuul/zuul): merge queues that keep `main` on a tested tree.
- [Overstory](https://github.com/jayminwest/overstory): multi-agent orchestration with a merge
  step.
- [awesome-agent-orchestrators](https://github.com/andyrewlee/awesome-agent-orchestrators): the
  list Air was checked against.
- [Metis](https://github.com/colliery-io/metis): how to break work into epics and beads.

## Docs

- [Design](docs/design.md)
- [Roles](docs/rules/roles.md)
- [Adopting Air](docs/rules/adopting-air.md)

## Status

In use on two repositories. Commands still change between releases, and `air install` tells an
installed repository what changed.

## License

Apache-2.0. See [LICENSE](LICENSE).
