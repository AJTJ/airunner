# Live trial request

This is the release trial from Air's design doc, section 9.1. `make trial` started you with
this text as your launch prompt; the same text is in `.air/trial-request.md` in the main
checkout, as the record. The workflow for this session is fleet, so offer no menu. Set up the
eleven scenarios below, file the beads they need, and let the fleet work through the ordinary
protocol in `.air/roles.md`. Do not direct workers beyond what a bead says. Nothing here is a
real product change. The verify is `make verify`; the fleet is the lane and worker-1 to
worker-3.

When every bead is closed, write one short report to `.air/trial-report.md` in the main
checkout, and stop. Per scenario: what happened against what was expected, with the event line
or sha that shows it. Then, for you, the lane and every worker (ask each one before you write): every retry,
every workaround or step done by hand, every refusal someone had to get around, every scenario
that only passed on a second try, and anything that confused anyone. For each, say why it was
needed, as far as anyone can tell. Write it from what you and the fleet saw
during the run. Do not spawn an agent to re-read the whole event log or the transcripts. Take every
count (beads, landings, refusals, pushes) from `air status`, `air audit` or the ledger, and name
the source beside it; do not count from memory or the bead list. The summary must not
contradict the list above it.

The copy's main already has the config the scenarios need: `"precheck": true` and the `serve`
lease in `.claude/air.json`, and the precheck `sh -n greet.sh` named in `CLAUDE.md`. Start by
filing beads.

## Scenarios

Unless a bead says otherwise, workers run the precheck before handing over.

| Scenario | What to file | Expected |
|---|---|---|
| Happy path | Three unrelated beads: add `farewell.sh` with its own test run by `make verify`; let `greet.sh` take a second name ("hello, Ada and Bo"); make `test.sh` print the number of cases that passed | One or more batches land on main, every bead is declared done through `air done` with proof and closes when the lane lands it, and `make verify` passes on main |
| Conflict | Two beads, filed together with no dependency between them, that change the same line of `greet.sh` to two different words. Say in both that the worker hands over without merging main (the lane merges main at the cut) | Each worker is warned once, by the hook, that the other holds `greet.sh`, and `air holdings` shows both; `air batch cut` drops one and names `greet.sh`; its worker resolves in its worktree and it lands in a later batch |
| Red batch | Once the conflict pair has landed, three beads filed together with no dependency between them. Breaker A changes the default greeting and does not update `test.sh`; breaker B changes what `farewell.sh` prints and does not update `test.sh`. Say in both that the break is the point, that the worker must not fix the test, and that after the red it waits two minutes, then reverts with a new commit and declares done again. The third is harmless: add a comment line to a third file. The three must be in one cut, and the fleet stop holds them together: wait until `air status` shows all three claimed, run `air fleet stop --reason "trial"`, ask nothing (`air fleet stop` refuses `air claim`, `air batch cut` and `air land` and nothing else, so the workers commit, run the precheck and declare done under it while the lane's cut is refused), and when `air status` shows all three batch-ready, run `air fleet resume` | While the stop holds, `air status` leads with `FLEET STOPPED`, the lane's `air batch cut` is refused naming the stop, and no session is killed. After the resume the lane cuts once and the batch has three members; it is red and the red reaches all three. `air record` lists all three as not yet verified alone with the first `--only` cut; the lane cuts each member alone in turn; the two breakers are red alone and held until their revert commits; the harmless one is green alone and lands; the list is empty before the lane cuts anything else; the reverts land in a later batch |
| Done before green | A bead whose worker is asked to run `air done` right after committing, before the lane's green, and then to make one more commit for the same bead (one more line in the same file) without running `air done` again | `air done` is accepted; `bd show` keeps the bead in_progress with the proof as a comment; the bead reads closed only after the `main moved` notice names it under `closed:`, at the landing that contains the declared head. The report records what happened to the later commit: which landing carried it, and whether anything named it. The design has not settled that case, so the report states what Air did |
| Lease | One epic with three children: a bead that adds `serve.sh` (prints "serving", sleeps 90 seconds, exits 0), then two beads, both depending on it, that each run `sh serve.sh > out-N.txt 2>&1` once and commit the output | The second worker is refused until the first releases the lease; it then takes it and runs. `air land` closes the epic at the landing of its last child and names it in the landing output |
| Dependency | Nothing extra: the two serve beads depend on the `serve.sh` bead. After the `serve.sh` worker has run `air done` and before its landing, a worker tries `air claim` on one of them | `air claim` is refused naming the dependency and its status, and `bd ready` does not list the dependents, until the landing puts `serve.sh` on main; after the `main moved` notice the dependents are offered by `beads are ready` and claimed, so neither serve run can start before `serve.sh` is on main |
| Behind main | Nothing extra: this happens when one branch waits while another batch lands | The waiting branch lands without its worker merging main |
| Landing by role | You, the coordinator, run `air land` once | It is refused; only the lane lands |
| Precheck | One bead, not one of the conflict pair, whose worker is asked to hand over without running the precheck, then to run it when the lane says it is missing | The branch is not cut until `air record precheck` passes |
| Start-up prompts | Nothing to file: the fleet that started with you is the set-up, and `make trial` answered the development-channels prompt in each pane. Read each pane once (`tmux capture-pane -p -t <session>`) and put in the report what each was waiting on | Every session shows in `air status` and no pane waits on a prompt, with nothing typed by you or the owner |
| Base branch | Last, once everything above has landed. The copy is scratch, made by `make trial`, and its main checkout (the directory above `.claude`) is yours to edit; nobody else does this step. Run `git branch -m main trunk` (from your worktree is fine; the main checkout's `HEAD` follows the rename), add `"base_branch": "trunk"` to `.claude/air.json` in the main checkout and commit it there on `trunk`, then file one bead that adds a line to `README.md`, saying in it that the base branch is now `trunk` | The cut, the green, the done and the landing name and move `trunk` (`trunk moved to` in the notice); `air status` prints `base branch: trunk`; the main checkout is still the owner's session, not a worker's |
