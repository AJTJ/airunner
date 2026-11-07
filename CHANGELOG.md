# Changelog

What changed in each release of Air. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow the rows in
`install::RELEASES`. `air install` prints the full list of changes a repository has not yet
been told about, including releases before this file started.

## [Unreleased]

## [0.4.15] - 2026-11-07

Live trial on 2026-11-06, the first with every session on its own pod
(`.air/trial-report-2026-11-06-pods.md`): 12 beads, 8 landings, one red reverted and landed
green, nothing typed by the owner after `TRIAL_PODS=1 make trial`. The 0.4.14 row below was
never tagged; this release carries both.

### Added

- `TRIAL_PODS=1 make trial` runs the trial fleet on pods: the copy gets a gitignored
  `.claude/air.local.json` of five targets, one replica and one path per session, built by
  `scripts/trial/pod-targets.py` from the owner's own overlay through this checkout's
  `scripts/kube-exec-nth`, after replica 1 has answered `hostname`; unset, nothing changes.
- `docs/research/remote-runner.md`: whether `air run` and `targets` are the idiomatic way to
  give a session a machine. The field puts the agent on the machine; Air's shape is CI's, right
  for the lane's verify and not for workers. Keep it for the lane, fix the two trial defects,
  build no more of it, and try one fleet on one pod next.
- The `user-summary` skill, the shape of every reply to the owner in chat: one line of
  state, then Question, Context and Recommendation per item, the settling action last. Ported
  from an adopter; CLAUDE.md names it and `plain-language` allows its three labels.
- `.claude/air.local.json`, gitignored by the adopter and read beside `.claude/air.json`
  from the main checkout: its top-level keys replace the committed file's, so a machine's
  `targets` and allow lists stay out of git history and take effect when written, not when a
  landing carries them (twice on 2026-11-05 a `targets` landing verified on the old target).
  `air status` names an unknown key the overlay set with its file, reports an overlay that
  does not parse, and warns once if git tracks it.
- `docs/research/folder-trust.md`: whether a fleet can start in a fresh directory without
  the owner answering Claude Code's folder-trust dialog. It cannot by any flag or setting; the
  trial script's `TRIAL_DIR` at a fixed, once-trusted path is the answer, and nothing is built.
- `docs/rules/remote-verify.md`, the setup guide for verifying on another machine: what a
  target is, which session gets it and why, what the machine needs, the gitignored files, the
  entry in both forms, the first run, the machine gone. Linked from the README, the docs
  index, the adopting guide and the design doc.
- `"targets"` entries take an `exec` list, the argv prefix that runs a command on the
  machine (such as `kubectl exec -i <pod> -c <c> -n <ns> --`); `air run` and `air record` run
  every step through it and push through it as git's `ext::` remote. `"kind": "ssh"` stays as
  the short form for `ssh user@host`.
- `docs/research/pull-over-exec.md`: how `kubectl cp`, the tar-over-ssh idiom and rsync
  carry files back over an exec transport and judge success. The 141 `air run --pull` sees is
  the local `kubectl` killed by SIGPIPE after the receiving tar stopped at the end-of-archive
  marker; the idiom's verdict is the receiver's, and a sender's non-zero exit code stays a
  failure. air-b50 follows it; nothing else is built.
- `air status` names each top-level key in `.claude/air.json` the running binary does not
  read, with the binary's version and build commit, and `--json` lists them as
  `unknown_config_keys`; a fleet ran a binary thirteen days behind main against a `targets` key
  and nothing said so.
- `worker_allow` and `coordinator_allow` in `.claude/air.json`: permission patterns the
  launcher puts on that role's `--settings` allow list, the lane taking the worker's, as the
  two deny keys already reach its deny list. For the adopter's own machine commands, which Air
  does not name; the deny list wins where both name a command.

### Changed

- the trial request's last scenario: the coordinator renames the scratch copy's `main` to
  `trunk` and sets `"base_branch": "trunk"` itself, so no row of `make trial`'s request waits on
  the owner.
- `roles.md`: the worker sequence says `git merge <base branch>` instead of `git merge
  main`, and the Every-role section says the name is `main` unless `base_branch` is set and that
  `air status` prints it.
- the ready notice: `<n> bead(s) are claimable; run `bd ready`` instead of a list of ids
  that was stale by the time a worker read it; the per-recipient rotation of that list is gone
  with it.
- `make trial` starts the trial fleet itself and needs no keystroke after it: the request
  text is the coordinator's launch prompt (`air coordinator -- "<text>"`, a positional that now
  leads argv instead of landing among the deny values), the development-channels prompt in each
  pane is answered by the script, and `TRIAL_DIR` defaults to one fixed path the owner trusts
  once.
- `air worker` allows `air run` and `air record` bare on its `--settings`, the way the
  lane's loop is allowed, so a worker's run against a target is not refused by auto mode's
  classifier; `.air/roles.md` tells workers to run both with no pipe or redirect.
- `.claude/air.json` gives the coordinator, the lane and each worker a target of its
  own, each pinned to a different replica of the dev deployment: `scripts/kube-exec-nth`
  resolves the session's pod when the command runs (coordinator, lane, worker-N, sorted by
  name), and a session
  whose replica is missing fails with one line naming it. The setup guide's new section
  "One machine per session" covers VMs reachable by ssh (one entry each, no resolver) and
  replicas of one deployment (air-oep).
- the committed `.claude/air.json` of this repository carries no `targets`: the five
  per-session entries, which name one laptop's kubeconfig and one user's home, live in the
  owner's gitignored `.claude/air.local.json`, so a clone on another machine runs locally
  until its owner writes an overlay of their own (air-vj6).
- `make trial`: it marks its scratch copy trusted in Claude Code's `~/.claude.json` itself
  (`scripts/trial/trust-copy.py`, one key for that one path), so no folder-trust dialog stops the
  trial on any machine. The launcher still refuses an untrusted repository.
- `closed-not-landed`: when the lane would not cut the branch either, the detail says why
  and names the fix (`not_batch_ready` on the row, `not cut by the lane:` on the printed line).

### Fixed

- the row `air land` writes at the landed sha carried no machine: it now copies the
  hostname, image, arch, nproc and cpu from the green it lands on, so a landing verified on a
  target names the machine like the verify did (first seen on the pod, 2026-11-05).
- a file a command wrote in the target checkout blocked the next checkout there of a
  commit that tracks it, so the precheck or the lane's verify recorded nothing (the pod trial,
  2026-11-06): `air run` and `air record` now run `git clean -qfd` and a forced checkout before
  the command, so the target holds only commits; ignored files there stay.
- `air selftest`'s base_branch probe landed without a git identity and failed with exit 128
  on a host with no global git config: every air the selftest spawns now carries the same
  identity its git fixtures set, from one constant.
- the lease check refused `air lease take X --reason "..." && <cmd>` as a whole; a take
  of the same resource joined to the command by `&&` now counts as holding it, and `;`, `||`
  or a take for another resource is still refused.
- `scripts/changelog-assemble.sh` folded a leading blank line under the first entry of a
  heading on mawk (Debian's awk), which made every verify on the Linux pod red on the changelog
  tests; the key is now tested before the element is referenced.
- `air run --pull` through `kubectl exec` reported "the tar stream failed" and recorded nothing
  pulled although the files arrived: the local tar closes the pipe at the end-of-archive marker
  and the prefix dies of SIGPIPE. The unpack is now the verdict; a sender exit code after a good
  unpack is reported as the target's tar failing.
- a repo's `coordinator_deny` patterns were appended after the Metis flags when `metis` is
  true, where the harness read them as positional arguments and denied nothing; they now go
  among the `--disallowed-tools` values. Worker argv was already right.
- The machine probe behind `air run` and `air record` read an empty CPU model on arm64 Linux,
  where `/proc/cpuinfo` has no `model name` line; it now falls back to `lscpu`, then to the
  CPU implementer and part, so the run row names the CPU on the pod.
- the drain test `a_real_hang_is_killed_and_reported` failed on Linux: its `sh -c "sleep 5"`
  forks `sleep` there, the kill reached `sh` alone, and the timeout path waited the whole
  stall for the pipes. The fixture now execs `sleep`, and the git.rs timeout path records the
  grandchild stall as a known gap with the condition for bounding the drain.
- twelve landing tests failed on a host with no global git config (`commit-tree` exited
  128, author identity unknown): the shared test helper now gives every spawned process its own
  git identity and no global config, so the suite passes and fails the same on every host.

## [0.4.14] - 2026-11-03

Live trial on 2026-11-03 (`.air/trial-report-2026-11-03.md`): 11 beads, 8 landings, one red as
planned. Six of eight scenarios passed on the first try. Landing by role was not shown at Air's
layer: the auto-mode classifier denied the coordinator's `air land` before Air could refuse it.
Precheck held, but the worker ran `air handover` itself instead of waiting to be told. One extra
red came from a worker amending a commit after the cut. No Air refusal was worked around.

### Added

- `base_branch` in `.claude/air.json`: the branch the fleet lands onto, `main` by
  default. Every gate, cut, landing, notice and refusal reads it where it used to say `main`,
  and names it; the session named `main` keeps its name. `air status` prints it when set and
  `--json` carries it. A selftest scenario runs a fleet whose only branch is `trunk` (air-h7l).
- On a Mac the launchers run the session under `caffeinate -i`, so an hour-long command no
  longer idle-sleeps the machine (three hours lost at an adopter on 2026-10-29); the assertion
  ends with the session. `air status` prints `idle sleep: <n> min` when `pmset -g` reports
  under ten minutes.
- `docs/research/coordinator-notices.md`: whether the coordinator's no-reply notices (main
  moved, batch result, session join) should go to a feed pane instead of its chat; the one-day
  ledger count, and the recommendation to log every channel push first and build nothing yet.
- `model` and `lane_model` in `.claude/air.json`: every launcher passes `--model` from them
  when the command line gives none, `air fleet up --model <m>` overrides for one launch, and
  `air status` marks a session whose transcript names another model.
- `docs/rules/adopting-air.md` names the fix for a file every branch appends to:
  `merge=union` in `.gitattributes`, for append-only files only.
- `air run -- <command>` runs a command on the calling session's target from `"targets"` in
  `.claude/air.json` (kind `ssh`: push HEAD to `refs/air/<session>` on the host, check it out, run
  there, stream the output back, pass the exit code through), and locally with nothing added
  when the session has no target.
- every `air run` is recorded in the ledger's `runs` table with the machine it ran on
  (hostname, `AIR_IMAGE` if the target sets one, arch, nproc, CPU model), beside the session,
  commit, tree, command, exit, times, output size and target; `air run --json` prints the row.
- `air run --pull <glob>` (repeatable): after the command exits, the matching paths under the
  target's checkout come back as one tar stream over the same ssh and are unpacked into the
  worktree, overwriting; the run's row names the pulled paths and their byte total.
- `air record` runs the check on the session's target when `targets` names one, on the
  pushed HEAD, and records it at the local HEAD as before with the machine stamped on the row;
  a dirty tree is refused and `--local` runs it here. Every recorded run now carries the machine
  it ran on.
- `air coordinator` allows `Bash(tmux send-keys *)` on its own `--settings`, so the
  start-up prompt answer the coordinator types after the owner's yes (air-d73) is no longer
  refused by auto mode's classifier; three refusals in two days of the live trial. Workers
  and the lane get no such rule.
- `docs/research/base-branch.md`: what merge queues and trunk-based development do about a
  base other than main, the count of `main` literals by sense, and the recommendation to
  document git's answer (a local `main` tracking any branch) and build no mechanism.

### Changed

- Surface notices are one file each under `crates/cli/surface.d/<ordinal>-<id>.rs`, folded
  into `install::SURFACE` by `build.rs`, so two beads adding a notice never conflict at the same
  line. A missing or empty directory fails the build; a malformed fragment is a rustc error
  naming the file.
- The coordinator's heartbeat is opt-in. `.air/roles.md` and the SessionStart context tell
  it to create one only where `.claude/air.json` sets `heartbeat_minutes`, and `air status`
  prints the expected interval when the key is set.
- The decomposition skill (installed as `air-decomposition`) carries four rules from Kairos,
  each cited into its repository: check every name a plan cites against the code before filing,
  cut a wide refactor as expand then contract, name a bead not yet filed by its full quoted
  title, and review a wave as a set before filing it.
- after a launch the coordinator reads each new pane and, if a start-up prompt is
  waiting, asks the owner once and on a yes answers it through tmux; the one exception to
  "do not type into panes". Folder trust stays the owner's. The design doc no longer says Air
  cannot answer the start-up prompts.
- The coordinator's branch is batch-ready only at a head it offered with `air ready` (new,
  coordinator only), unless `.claude/air.json` sets `coordinator_autobatch` true; `air status
  --json` says `not-offered` until then. Every coordinator commit used to cost the lane a cut.
- The remote runner's push over a command prefix (`kubectl exec`) is decided: git's `ext::`
  remote helper, not a bundle piped through the prefix; one row in the design doc's technology
  decisions carries the measurement.
- the coordinator's first step, unless the owner has said how to work, is to offer four
  named workflows in plain text (fleet, coordinator alone, mixed, read-only) and take the pick
  for the session; "commit in your own worktree only when no worker can make the change" is
  replaced by the one real case, a commit only the coordinator's branch can carry.
- `air worker`, `air lane`, `air coordinator` and `air fleet up` pre-answer the start-up
  prompts Claude Code documents an answer for (`--add-dir <main checkout>`, `--permission-mode
  auto`, `skipAutoPermissionPrompt`) and refuse to launch, naming the fix, when the main checkout
  has no `hasTrustDialogAccepted` in `~/.claude.json`. Air never writes that key.

### Fixed

- the `air record` kill test polls with one connection and a short sleep, with a 60 s
  deadline, so a loaded lane does not red the batch on a test's wall clock.
- A worker branch that is green at a head containing `main` is batch-ready under a lane, so
  the lane cuts it instead of leaving it closed and unlanded.
- `air batch cut --only` and `--without` could not split a red batch: its members were
  already held out of the set the split chose from. A split now chooses among the batch-ready
  branches and the newest red batch's members at their red-held heads.
- the three batch_cut tests reach their assertion when a cut fails and print the command's
  stderr in the red, so a git error behind an empty member set names itself.
- `air land --help` had no description and `air ready --help` showed the landing's: the
  `Ready` variant had been inserted under `Land`'s doc comment (air-m8u). Each shows its own.

## [0.4.13] - 2026-10-22

Live trial on 2026-10-22 (`.air/trial-report-2026-10-22.md`): 11 beads, 7 landings, one red as
planned. Six of eight scenarios passed on the first try; conflict passed on a re-filed pair, and
lease passed with the refused worker giving up its bead as the refusal told it to. Released on
the owner's go.

### Added

- `air doctor` compares Dolt against a pinned 2.3.5 when bd runs in server mode, and reports
  a mismatch without refusing. Air's install advice for bd and Dolt now points at their GitHub
  releases for the pinned versions, since Homebrew only has the latest.

## [0.4.12] - 2026-10-21

Live trial passed 8/8 on 2026-10-21 (`.air/trial-report-2026-10-21.md`): 11 beads, 7
landings, one red as planned. The first release with prebuilt binaries.

### Added

- Release binaries. Pushing a version tag builds `air` for macOS (Apple Silicon and Intel)
  and Linux (x86_64 and arm64) with cargo-dist, and attaches them to a GitHub Release with a
  shell installer, so installing Air no longer needs Rust or a checkout.
- `air batch cut --without <worker>...` and `--only <worker>...`, so the lane splits a red
  batch with a recorded cut instead of merging by hand; leaving everyone out exits 1.
- The decomposition skill (installed as `air-decomposition`) has a "Bead shape" section: every
  part of a bead, who reads it, what enforces it, and the `.beads/config.yaml` setting that makes
  bd require a section. The README and adopting-air link it.
- `air install --write` writes Air's design doc, its register of solved issues and the
  adoption guide to `.air/docs/` from the binary, ships the `air-defect` skill that reads them,
  and `air doctor` names a copy that differs. `air release-check` refuses a release whose
  newest parity sweep predates the last change to `crates/` or `docs/rules/`.
- A `defect` skill for this repository: a defect, finding, red, refusal or capture is
  explained in seven parts (what the parts do, what happened, why, the lifecycle state, precedent,
  fix and cost, what the fix could break), with a worked example. CLAUDE.md makes it the rule, and
  the coordinator's roles text asks the same shape for a capture triaged to a bead.

### Changed

- Docs are at parity with 0.4.11: README, CLAUDE.md, design.md, roles.md, adopting-air.md,
  the skills and the trial count script no longer describe removed behaviour.
- The live trial's request says every count in its report comes from `air status`,
  `air audit` or the ledger, with the source named, and that the summary must not contradict
  the friction list.
- `make trial` commits the scenarios' config (precheck, the `serve` lease, the precheck line in
  `CLAUDE.md`), so the trial request has no setup step and no session edits a config file.
  `air install --write --remove-stale` removes a hook `air install` reports as STALE HOOK instead
  of asking for `.claude/settings.json` to be edited by hand.
- The `main moved` notice no longer tells a worker `git merge main` brings the landing into
  its branch. Workers read it as an instruction and merged mid-batch, losing a batch cycle; the
  notice states what landed and which files changed, and the worker decides.
- The repository's history was rewritten to drop the `Claude-Session:` line from 604 commit
  messages, so every commit hash and tag before 0.4.12 changed. A clone made earlier needs a
  fresh clone or `git fetch && git reset --hard origin/main`.

### Fixed

- Every notice to workers and the lane (beads ready, batch-ready, the queue empty) was
  produced only by the coordinator's channel poll, so nobody was told anything while that
  session was stopped. `air record`, `air claim`, `air release` and `air close` now queue them
  after their write; the poll still runs as a sweeper, and each notice is sent once.
- After a red batch with several members, Air offered every member for the next batch again,
  so the branch that broke it was verified red a second time; every member now waits until it
  has a new commit or a later green contains it.
- `make changelog` refused every fragment after a release, because the release leaves
  `[Unreleased]` with no headings; a missing heading is now added under `[Unreleased]` in the
  standard order.

## [0.4.11] - 2026-09-29

The round on an adopter's 0.4.10 findings: 42 landings, live trial passed 8/8 on 2026-09-29
(`.air/trial-report-2026-09-29.md`). 0.4.10 was never tagged; its notices ship here.

### Added

- Two install notices the round's sweep found missing: the roles text and air-* skills
  caught up with the round (air-4l5w), and `air bd-server up` checking its database before
  the port (air-df53).
- `make changelog` folds the `changelog.d/` fragments into `CHANGELOG.md` under
  `[Unreleased]` and deletes them; `make release` refuses while any is left.
- The `air-do-less` skill has a "Precedent before change" step: before any change, read the
  solved-issues register and the lifecycle table and write a `## Precedent` section on the bead
  (register entries, states touched, weight, the surgical change). `air-decomposition` says
  the coordinator writes it at filing. This repo sets `lint.sections` in `.beads/config.yaml`
  so `bd create --validate` refuses a task or bug without the section.
- `crates/cli/src/cmd/lifecycle.rs`, a model test that drives the batch-ready rule, the cut's
  refusal, the batch scan and the close gate through every order of commit, cut, verify, land,
  close, merge and a move of main for two workers and a lane, and fails if any reachable state
  can no longer land and close every bead. `docs/design.md` §6 opens with the lifecycle table
  it checks.
- `docs/solved-issues.md`: the register of every problem Air has hit and demonstrably
  solved, with the fix and what would regress it (owner, 2026-09-28).

- `air status` shows, per worker with a precheck in the last day, `precheck p50 Ns, wait after
  p50 Ns`: how long its prechecks ran and how long from a run's end to its next tool call.
  `air audit` prints the totals. The ledger gains `verify_runs.resumed_at` (schema v26).
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

- the README, roles, adopting-air, the docs index, the trial request and the beads,
  decomposition, do-less and project-diligence skills say what the 2026-09-28 round built: no
  fence, eight trial scenarios, the lane's allowed loop, the claim refusals, `-n 0` on bd reads,
  bd's Dolt server run by Air, and where the verify-lane landing waits.
- `air close` refuses a bead that no commit names, on the branch or on main, even on a
  green head, and names the fix: commit the result, a document if nothing else, with the
  `Bead:` trailer. A green head was the one path by which a bead with no commit closed.
- `docs/design.md` is rewritten for the end of the 2026-09-28 round, at under 600 lines:
  it describes only what exists, opens §6 with the lifecycle table every flow follows, lists
  today's standing rulings, and points to `docs/solved-issues.md` for what was removed and why.
- this repo sets `verify_lane: true`: the lane verifies once per batch and workers run no
  verify of their own (four parallel Rust verifies had put the owner's machine at load 40).
- A bead writes its changelog line as a fragment, `changelog.d/<bead-id>.md`, instead of into
  this file, which every landing conflicted on. This is the last line written here directly.
- `air land` closes an epic once bd reports every one of its children closed, and never an
  epic with no children. `air status` names an epic to decompose only when it has no
  children, and prints the beads-without-initiative count only when `"metis": true` is set.
  The decomposition skill puts an epic's "Done when" in its last child and files children
  with their `--deps`.

- The channel poll no longer runs `lsof`. Five sessions polling every few seconds kept three
  or four `lsof` scans running at 40-50% CPU each on a loaded machine. Only `air status` run by
  a person and `air land` read processes; the poll uses their last reading while it is under
  30 s old, and otherwise treats the tree's readers as unknown.
- The project's Dolt server runs as a plain background process in its own process group, with
  its pid in `.air/dolt/server.pid` and its output in `.air/dolt/server.log`, instead of in a
  tmux session `<project>-dolt`. `air bd-server status` names the pid. This also stops a
  scratch `air init --write` leaving a session and a server on the owner's own tmux.
- Air's messages state facts and the command that acts on them, with the opinions taken out
  (air-ch9p). `air status` prints `in batch: <worker> at <head> is in batch <sha>, being
  verified now` for a head inside a running batch, instead of listing it as batch-ready. The
  coordinator is not sent `landable` while a lane session is live. A close refused for want
  of a trailer names the `Bead:` trailer. Fixes are printed as `fix: ...` rather than wrapped
  in "run `...`", and none suggests `git commit --amend`. A batch of one has its own green and
  red text. "Close them now" names only beads the member still holds, and several messages
  lose "most likely", "prompt them", "yours" and "closed is closed".
- `air audit` prints, for every budget, the recorded median and maximum beside the budget and
  ten times the maximum, which the next release sets Air's numbers from; each budget names the
  measurement or the ceiling behind it. A wait that reaches its budget reads `over budget`.
- The `beads are ready` notice and a claim refused because a peer holds the bead now point
  at `bd ready` for the next pick: the notice's list is a snapshot, and workers walking it
  after a hand-over caused most refused claims in the 0.4.10 trial.
- Under a verify lane, a worker whose every claim is batch-ready is nudged with the ready beads
  when it stops, the same as a worker with no claim.
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
- `air release --reason` and `air reclaim --reason` take any text. The fixed words are now
  the documented common values.
- The `beads are ready` notice lists the same beads to every idle worker, rotated within each
  priority group by the recipient's position, so idle workers do not all try the same first
  bead. The text says when the order was changed.

### Removed

- The two fallbacks dated 2026-08-23: a commit with no `Bead:` trailer is no longer attributed
  from its message, and a digest with no `bead:` front matter no longer counts by its file name
  and time. `air doctor` no longer reports dated rules, since none are left.
- The worktree fence: a worker's Edit or Write outside its worktree is no longer denied. Its
  only recorded refusals were writes to the session's own scratchpad. The roles text tells a
  worker to work only in its worktree and use its scratchpad for the rest.
- The `landed-not-closed` condition, and the per-clause verdicts in `air land` it came from.
  `air land` still prints each landed bead with its acceptance text.
- `air batch cut` no longer tells a worker its branch was held out for a missing precheck, and
  writes no `no-precheck` line. `air status` still names `no-precheck` for such a branch.
- `air record` no longer labels a run `suspicious` or `flaky-at-head`. It prints the green and
  red counts at the commit instead, and the close refusal states those counts rather than
  calling the test flaky.
- The Stop hook no longer reports what a close would refuse. `air close` and `air handover`
  still say it when they are run.
- The hook no longer reads `bd close` or `bd update -s closed|awaiting_review` from a shell
  command: neither the hand-over check on it nor the claim release after it. Workers and the
  lane close with `air close`, and their launchers deny bd's write commands.
- `air release --worker`. The coordinator uses `air reclaim` instead.
- The `phase-transitions` skill is no longer installed. `air install --write` removes it.
- Most of the general-purpose skills this repository carried; it keeps Air's own and the few
  used to work on it.

### Fixed

- `air fleet up`, `air worker` and `air lane` started a second session in a worker's worktree
  when its tmux session had another name (after `"project"` changed). A worker the ledger has a
  live session for is now left running, with `<name>: running (session <id>)`.
- Four statements in `docs/design.md`: `air land` is refused only by a verify in the main
  checkout or the lane's tree; `air batch cut`'s other guards are named in the lifecycle table;
  the lifecycle model names which decision functions it calls; and the timing budgets are
  described as the code keeps them, each naming its measurement, unchanged pending air-1cdr.
- Tests that waited a fixed ten seconds for a freshly written script or binary went red under
  fleet load, while macOS checked the new executable. The bd-server probe now runs with no
  real server, and the launch, init and MCP tests allow up to 60 or 120 seconds.
- A branch whose bead was closed on a green, and which then fell behind main before the lane
  landed it, could neither land nor join a batch, because batch-ready needed a bead the worker
  still held. A closed bead now counts, and `air status` names it `closed, waiting for the lane`.
- Three tests that timed Air against a stalled child asserted a fixed number of seconds and
  went red on a loaded machine. Each now asserts that the command finished before the stall
  it injects, which is the fact under test. The status test's seeding call also gets a bd
  budget no loaded machine reaches.
- `air batch cut` with no members exited 0, so `air batch cut && air record verify` recorded a
  green at bare main. It now names what held each branch and exits 1; a dry run still exits 0.
- `air handover` and `air close` said only "no green" at a head where a red was recorded, and
  where the repo gates batches on a precheck that had not run. They now name the red with its
  kept output, and the missing precheck with `air record precheck`.
- A branch whose red batch had it as the only member stayed ready for the next batch, so the
  lane merged the same red again. Such a branch is now left out until its worker commits past
  it.
- `air batch cut` refused for good once the lane's green batch fell behind main, and `air land`
  refused that batch too, so the lane could neither cut nor land. The cut now refuses only a
  green batch that could still land; a stale one is named and re-cut from main.
- A `bd_server` unit test read a neighbour listening on its port as a running server and
  failed under a loaded verify. `start` now checks for Air's database before it probes the
  port, and the test holds a listener on its own port to prove the port does not matter.
- A recorded edit never expired, so the peer warning and `air holdings` named edits days old
  and long landed, and sessions long gone. An edit now ends once the file is clean in its
  holder's worktree and unchanged against main.
- `air claim` took a bead that an open `blocks` dependency keeps out of `bd ready`. It now
  refuses, naming the blocker and its status.
- Seven hard-coded limits that changed an answer without saying so are gone. The close gate
  checks every green recorded after the bead's oldest commit, not the newest 12. The model
  column reads the transcript up to the first model line, not the first 64 KB, so launched
  sessions no longer show `[?]`. `bd ready` and `bd list --status` pass `-n 0`, so more than
  100 ready beads or 50 in progress are all seen. The loop times read every run in their
  window, `air init` walks the whole tree, and bead ids in prose have no length or count cap.
  Red run logs are no longer pruned to the newest 20.
- A worker that joined, or finished its claim, while beads were already ready was never told
  them, because "beads are ready" went out only when the ready set changed. It is now told the
  set as it stands, once.
- A close in the seconds between a landing moving main and its record being written was
  refused. The gate now asks git whether every commit naming the bead is in main. The "batch
  green" notice now names only the beads the batch fully contains, and a member with no such
  bead is not told to close anything.
- A batch cut stopped by a leftover `index.lock` left no record, and a verify stopped by hand
  left no run. A failed cut now writes `batch-cut / failed` with its reason and who holds the
  lock, and `air record` survives SIGINT, SIGTERM and SIGHUP to write the run as interrupted
  with its duration, which `air status` shows under the worker.
- `air fleet up` logged and printed that it had started the fleet when every launch was
  refused. It now names each launch that did not start and why, and its event reads
  `incomplete` rather than `up`.
- The coordinator could put a question prompt to the owner and wait on it for hours, hearing no
  notices; an adopter's coordinator sat in one for six hours. It is now denied the prompt, like
  workers and the lane, and roles.md says a question for the owner is a bead labelled `owner`.
- The coordinator's channel pushed the same condition on every poll when one worker had two
  beads in it: an adopter's coordinator got `closed-not-landed` 417 times in a day. Pushes
  are now keyed by the bead, branch or lease they are about, and a change-only condition's
  age counts from when that subject first appeared. The Stop nudge names the same ready set
  once per session, and `air land` warns about the same main-checkout readers once.
- A session whose pid had been reused, or had become a zombie, read as alive and got most of
  the ready notices. A session is now alive only when its pid runs with the start time it
  registered with; `air status --json` shows `liveness` as alive, dead or unknown, and only
  live sessions hear ready notices or raise idle-without-claim. A new session under a name
  replaces that name's earlier rows, and the main checkout's session shows as the owner.
- The lane piped `air land` into `tail`, the auto-mode classifier refused it, and the session
  then refused every later landing. The lane's settings now allow its whole loop (`air land`,
  `air batch cut`, `air record`, `air status`, `git merge`, `git reset --hard main`), and the
  roles text tells it to run them bare.
- `air audit` said 0 closes passed while beads were closing, counted `uds:` messages to live
  sessions as unresolved, counted one subject for many beads or heads, and described the Stop
  nudge as naming beads once beside a count of its repeats. All four now match the ledger.
- `air land` refused while any verify ran anywhere in the fleet, so a lane's green batch could
  wait indefinitely behind workers' own runs. Only a verify in the main checkout or the lane's
  tree refuses it now.
- `air close` and `air handover` ran the whole hand-over check on an id like `air-1 notes.md`
  and advised a `Bead:` trailer naming both words. An id with whitespace is now refused naming
  it, and when the check would refuse, an id bd does not have is named instead.
- `make verify` was red when run from a fleet session, because the tests that spawn `air`
  passed on the session's `AIR_ROLE` and the other launcher variables. Every spawn in
  `crates/cli/tests` now goes through one helper that removes `AIR_*` and `BEADS_ACTOR`.
- `air status` kept showing `STOPPED at <t>` after the session resumed; it now shows only
  until the session's next event of its own. A session in a permission or idle prompt shows
  `waiting for the person since <t>`, a subagent's hooks no longer mark its parent idle, a
  stop with no reason says so instead of naming an API error, and no age reads negative.
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
- `landed-not-closed` was raised for a bead `air close` had already closed, and stayed. A bead
  whose claim the ledger records as closed is no longer reported.
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
- A branch `air batch cut` dropped for a conflict was offered to the lane again before its
  worker had committed, and dropped again. A dropped head is not batch-ready until it moves.
- The drop notice said "merge main" for a conflict with a branch not on main yet, and a worker
  merged the peer's unlanded commit. It now names the branch and its sha and says to wait for
  it to land, then merge main. The cut's output says the same, and no longer tells the lane to
  name each drop itself.
- `air batch cut` wrote a `batch-held` notice for a head whose precheck had just been recorded
  green. It re-reads the record before writing the hold.
- `air handover` printed "NOT landable" beside "the close would pass" for work already on
  main. It now says the head is already on main.
- `air land` settled no acceptance clause naming a bare filename such as `serve.sh`. A bare
  filename counts as a path when the landed tree has it.
