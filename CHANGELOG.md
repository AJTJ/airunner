# Changelog

What changed in each release of Air. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow the rows in
`install::RELEASES`. `air install` prints the full list of changes a repository has not yet
been told about, including releases before this file started.

## [Unreleased]

The next release is 0.4.2. It is tagged once a live trial of it passes (see
`docs/design.md` §9.1). 0.4.0 and 0.4.1 were never tagged: their trials found defects, which
are fixed below.

### Added

- The verification lane is its own role. `air lane` starts it, and only the lane lands.
- Every role runs in its own worktree and tmux session, named `worker-<N>`, `lane` and
  `coordinator`, with the project name in front.
- `air coordinator` asks whether to start the fleet. `air fleet up` starts the lane and the
  workers (`"workers"` in `.claude/air.json`, 3 by default).
- `air batch cut` builds the lane's batch: a conflict check on every pair of branches, then a
  merge of main and each member, dropping and naming any branch that conflicts.
- `air record precheck` records a worker's quick check. With `"precheck": true`, a branch is
  ready for the lane only after a passing precheck.
- `"leases"` in `.claude/air.json` lists the commands that need a shared resource. Air refuses
  them to a worker that does not hold the lease, and `air lease needs "<cmd>"` explains why.
- Notices from Air to the fleet: idle workers hear when beads are ready, the lane hears when a
  branch is ready, workers hear their batch's result or that their branch was dropped, and a
  worker hears when a lease it wanted is free.
- `air fleet stop` and `air fleet resume` stop and resume all work with one command.
- `air status` shows what else is running in each worktree, and warns about a session running
  in the main checkout.
- `air install --write --pin` runs a repository on its own copy of `air`, so a new build can be
  tried without replacing the installed one. Any `air` 0.4.1 or later hands off to the pin.
- `make trial` prepares a pinned copy of `examples/minimal` for the live trial.
- `make adoption-check` adopts `examples/minimal` from scratch at every release.
- `examples/minimal`, a three-file project showing a check and every file Air adds.
- The decomposition skill asks every bead to end with a `## Context` section naming the skills
  to load and the files to read first.

### Changed

- The fleet's protocol lives in Air's roles text (`.air/roles.md`). An adopting repository keeps
  only its own commands, setup and resources.
- A branch no longer has to contain main to be ready for the lane; the lane merges main in.
- `air init` proposes the check the project already has (`cargo test`, `npm test`,
  `make test`) and names the project after its directory.
- Air's records live in `.air/`, which git ignores. Journals go in `.air/journal/`. Digests are
  optional (`"digests": true`) and go in `.air/digests/`.
- `verify_lane` in `.claude/air.json` is now `true` or absent.
- Time limits are generous: bd calls get 60 seconds plus 5 seconds per bead, and channel tool
  calls get as long as the command they run.
- Workers no longer message the coordinator when they close a bead, and the coordinator no
  longer hands out beads by message.
- `--print` on the launchers writes nothing.
- Every decision Air makes writes an event line and is counted by `air audit`.

### Removed

- The `phase-transitions` skill is no longer installed. `air install --write` removes it.

### Fixed

- In a pinned repository, sessions ran the `air` on PATH instead of the pin.
- The lane's `air land` was refused by Claude Code's auto mode.
- `make trial` did not turn on the verification lane.
- Every session stopped at the MCP server approval prompt.
