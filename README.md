<p align="center">
  <img src="assets/logo.svg" alt="AIRunner" width="640">
</p>

Multi-agent accountability and verification system. Lighter than air.

## Philosophy

Do less. Most agentic systems try to do too much. The model knows how to do the work and keeps getting better at it, so Air does not plan
the work or tell agents how to do it.

## What Air is

- **A merge queue.** Workers finish branches, and a verification lane merges them into one batch
  and lands it on `main`.
  - Your project decides what passes, for example `make verify`.
  - The lane runs it once per batch, and Air records which commit passed.
  - A bead closes only on a passing run.
- **Sessions.** Air starts the coordinator, the workers and the lane, each in its own git
  worktree and tmux session.
- **Shared resources.** Leases for anything one agent can use at a time, such as a port or a
  simulator.

## How you use it

You talk to the coordinator. It writes the beads, workers do them, and you check the result.
Every session is a full harness running in a worktree, in tmux that you can interact with.

## Upcoming

- Support for [Pi](https://github.com/earendil-works/pi) and other open-source harnesses.
- Messaging between agents, managed by Air.

## Adopt it

Needs git, Rust, tmux, Claude Code and [beads](https://github.com/gastownhall/beads) (`bd` 1.2.2).

```sh
cargo install --path crates/cli     # from a checkout of this repository
cd /path/to/your/repo
air init --write
```

Put your real check in `make verify`, commit, and run `air coordinator`. If agents share a port or
a device, list the commands that use it under `leases` in `.claude/air.json`.

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
