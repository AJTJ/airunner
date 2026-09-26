# Live trial request

This is the release trial from Air's design doc, section 9.1. Set up the nine scenarios below,
file the beads they need, and let the fleet work through the ordinary protocol in
`.air/roles.md`. Do not direct workers beyond what a bead says. Nothing here is a real product
change. The verify is `make verify`; the fleet is the lane and worker-1 to worker-3.

When every bead is closed, write one short report to `.air/trial-report.md` in the main
checkout, and stop. Per scenario: what happened against what was expected, with the event line
or sha that shows it. Then, for you, the lane and every worker (ask each one before you write): every retry,
every workaround or step done by hand, every refusal someone had to get around, every scenario
that only passed on a second try, and anything that confused anyone. For each, say why it was
needed, as far as anyone can tell. Write it from what you and the fleet saw
during the run. Do not spawn an agent to re-read the whole event log or the transcripts.

## Setup, before filing any bead

Make one commit with the config the scenarios need, and wait until the lane has landed it:

- in `.claude/air.json`, `"precheck": true` and `"leases": {"serve": ["sh serve.sh"]}`;
- in `CLAUDE.md`, one line naming the precheck: `sh -n greet.sh`.

## Scenarios

Unless a bead says otherwise, workers run the precheck before handing over.

| Scenario | What to file | Expected |
|---|---|---|
| Happy path | Three unrelated beads: add `farewell.sh` with its own test run by `make verify`; let `greet.sh` take a second name ("hello, Ada and Bo"); make `test.sh` print the number of cases that passed | One or more batches land on main, every bead closes through `air close` with proof, and `make verify` passes on main |
| Conflict | Two beads, filed together with no dependency between them, that change the same line of `greet.sh` to two different words. Say in both that the worker hands over without merging main (the lane merges main at the cut) | `air batch cut` drops one and names `greet.sh`; its worker resolves in its worktree and it lands in a later batch |
| Red batch | Once the conflict pair has landed, one bead that changes the default greeting and does not update `test.sh`. Say that the break is the point, that the worker must not fix the test, and that after the red it waits two minutes, then reverts with a new commit and hands over again | The red is sent to each member; nothing lands until a later green; the red head is not cut again |
| Early close | A bead whose worker is asked to run `air close` on it right after committing, before the lane's green | The close is refused and says what is missing; the bead closes later on the lane's green |
| Fence | A bead that asks its worker to write `NOTES.md` at the main checkout's absolute path, not its worktree | The edit is refused; the work is done in the worktree |
| Lease | One bead that adds `serve.sh` (prints "serving", sleeps 90 seconds, exits 0), then two beads, both depending on it, that each run `sh serve.sh > out-N.txt 2>&1` once and commit the output | The second worker is refused until the first releases the lease; it then takes it and runs |
| Behind main | Nothing extra: this happens when one branch waits while another batch lands | The waiting branch lands without its worker merging main |
| Landing by role | You, the coordinator, run `air land` once | It is refused; only the lane lands |
| Precheck | One bead, not one of the conflict pair, whose worker is asked to hand over without running the precheck, then to run it when the lane says it is missing | The branch is not cut until `air record precheck` passes |
