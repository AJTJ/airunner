# Changelog

What changed in each release of Air. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow the rows in
`install::RELEASES`. `air install` prints the full list of changes a repository has not yet
been told about, including releases before this file started.

## [Unreleased]

The next release is 0.4.6. It is tagged once a live trial of it passes (see
`docs/design.md` §9.1). 0.4.0 to 0.4.4 were never tagged, and 0.4.5 was tagged but its trial failed: their trials found defects or
missing notices, which are fixed or added below.

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
- When main moves, the coordinator and every worker hear which beads landed and which files
  changed.
- The coordinator hears each worker's capture at once, and hears once when the ready queue runs
  empty while a worker is idle, with the epics that have no open child.
- `air status` shows what else is running in each worktree, and warns about a session running
  in the main checkout.
- `air install --write --pin` runs a repository on its own copy of `air`, so a new build can be
  tried without replacing the installed one. Any `air` 0.4.1 or later hands off to the pin.
- `make trial` prepares a pinned copy of `examples/minimal` for the live trial.
- `make adoption-check` adopts `examples/minimal` from scratch at every release.
- `air init --write` sets a new project's bd up in server mode, on a Dolt server of its own
  with its data in `.air/dolt/`. A project that already has `.beads/` is not changed.
- `air bd-server up` starts that server when it does not answer, and `air fleet up` and the
  launchers do the same before starting sessions. `air bd-server status`, `air doctor` and
  `air status` show bd's mode and whether the server answers.
- The coordinator's channel restarts a bd server that stopped and tells the coordinator once.
- `examples/minimal`, a three-file project showing a check and every file Air adds.
- The decomposition skill asks every bead to end with a `## Context` section naming the skills
  to load and the files to read first.
- `air reclaim <id> --worker <name> --reason <r>` lets the coordinator take back a gone
  worker's bead through `bd reclaim`. bd lets go only after the worker's claim lease runs out,
  five minutes after the claim; until then the command says when and exits non-zero.

### Changed

- Air expects bd 1.3.0 instead of 1.2.2 (`air doctor`'s pin, the install hint, the docs).
  bd 1.3.0 refuses the reopen `air release` makes on a bead someone else holds; see Fixed.
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
- The coordinator does not implement. Its roles text says to file the work as a bead for a
  worker and to commit in its own worktree only when no worker can make the change.
- Workers and the lane close with `air close <id> --reason-file <proof>`. It runs the hand-over
  check on each bead before closing and closes nothing if one fails. It is no longer denied to
  workers, and the roles text no longer tells anyone to run `bd close`.
- The hook's check on a raw `bd close` stays as a backstop, and its refusal now names
  `air close`. `air handover` names the `air close` that would pass.

### Removed

- `air release --worker`. The coordinator uses `air reclaim` instead.
- The `phase-transitions` skill is no longer installed. `air install --write` removes it.
- Most of the general-purpose skills this repository carried; it keeps Air's own and the few
  used to work on it.

### Fixed

- `air release` passes the worker's actor to bd, which bd 1.3.0 requires to unassign a bead.
  Without it a session whose `BEADS_ACTOR` was not its claim's actor could not give its own
  bead back.
- In a pinned repository, sessions ran the `air` on PATH instead of the pin.
- The lane's `air land` was refused by Claude Code's auto mode.
- A worker's `bd close` written on its own line after a commit in the same shell call was not
  checked, so a bead closed with no green containing its commits. Closing through `air close`
  runs the check whatever the command line looks like.
- `make trial` did not turn on the verification lane.
- Every session stopped at the MCP server approval prompt.
- A freshly started worker was never counted as idle, so it was never told that beads were
  ready and the fleet stalled.
- `air land` refused a lane batch of only the coordinator's commits because they named no
  bead, after the lane had cut and verified it. It now lands carrying no bead. The lane is no
  longer told to `git commit --amend` a refused batch, which would discard its green.
