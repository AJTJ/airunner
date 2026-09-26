# Changelog

What changed in each release of Air. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow the rows in
`install::RELEASES`. `air install` prints the full list of changes a repository has not yet
been told about, including releases before this file started.

## [Unreleased]

The next release is 0.4.8. It is tagged once a live trial of it passes (see
`docs/design.md` §9.1). 0.4.0 to 0.4.4 were never tagged, and 0.4.5 and 0.4.6 failed their trials, and 0.4.7 passed but was not put out: their trials found defects or
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
- `air handover` names the `air close` that would pass.
- Workers and the lane read beads and use `bd comment`; every bd command that changes a bead
  or the store is on their deny list.
- Only the coordinator is told at session start to create its 5-minute wake.
- `air install --write` adds `"project"` to a `.claude/air.json` that has none, set to the main
  checkout's directory name.
- `make trial` stops the previous trial copy's `<project>-dolt` tmux session and removes that
  copy before making a new one.

### Removed

- The hook no longer reads `bd close` or `bd update -s closed|awaiting_review` from a shell
  command: neither the hand-over check on it nor the claim release after it. Workers and the
  lane close with `air close`, and their launchers deny bd's write commands.
- `air release --worker`. The coordinator uses `air reclaim` instead.
- The `phase-transitions` skill is no longer installed. `air install --write` removes it.
- Most of the general-purpose skills this repository carried; it keeps Air's own and the few
  used to work on it.

### Fixed

- After a red batch, the next `air batch cut` built on the red merges and would have carried
  them into the next batch. Every cut now resets the lane's branch to main first. It refuses
  while the lane has uncommitted changes, or while its head is a passing batch not yet landed.
- A worker's close was refused as behind main when its own batch had just landed, although
  main contained its work.
- A worker's close was refused as behind main when the bead had landed or had a green, but the
  worker had already committed its next bead on top. The gate now judges the bead's own
  commits, not the branch head.
- A batch of one sent its member no result, because its members were read from merge commits
  and it made none. `air batch cut` now records its members.
- A branch whose batch went red at its current commit stayed ready, so the lane cut it again.
- The lease check missed a leased command followed by a redirection such as `2>&1`.
- `air record precheck` flagged a correct precheck `suspicious` when it printed nothing.
- `air init --write` sets `dolt.auto-start: false` in `.beads/config.yaml`, so bd no longer
  starts its own empty Dolt server on the project's port while Air's is down.
- `air bd-server up` and `air bd-server status` check that the process on the port serves this
  project's data. When a different process answers, they name it and start nothing.
- `air bd-server up` picks and writes a port when `.beads/dolt-server.port` is missing, instead
  of failing.
- Every Air session runs with `BEADS_DOLT_AUTO_START=0`, and `air status`, `air doctor` and the
  coordinator's keep-alive report a port held by another process as not this project's server
  instead of up; the keep-alive starts nothing on it and tells the coordinator once. Workers
  and the lane are denied `bd ready --claim`, `bd orphans --fix` and `bd events prune`.
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
- A branch `air batch cut` left out for a missing precheck was never told so. Its worker now
  hears it once per head.
- Under a lane, `air close`, `air handover` and the Stop hook told a worker waiting on its
  batch to `git merge main`. They now name `main-merged` only beside a green at the head that
  lacks main, and the Stop hook says nothing while only the lane's checks are missing.
- `air land` warned on every landing about Air's own bd server. Processes whose working
  directory git ignores are left out.
- The `landable` condition was raised for the lane's own branch and offered the coordinator
  `air land --all`, which it may not run. It skips the lane's branch and names who lands.
- Main read `not green` after every landing. `air land` records the landing commit green when
  its tree is the verified one, and a landing with no bead has no trailing colon in its title.
- A lease refusal for `air lease take X && <cmd>` now says to take the lease in its own call.
- A claim that lost a race was described as a left-over assignee. It now says another worker
  most likely just took the bead and to take another.
- Messages that described the old setup were corrected: the fence refusal no longer calls the
  main checkout the coordinator's, the claim refusal names `air reclaim` instead of the removed
  `air release --worker`, and an in-flight landing no longer reads as verifying with a rollback
  armed.
