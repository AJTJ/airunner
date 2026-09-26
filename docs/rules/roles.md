# Roles: worker, lane and coordinator

You read this at session start. It is the fleet's whole protocol: how work moves from a claim
to main, for every role. The repo's CLAUDE.md holds only what is the repo's own: its domain
rules, its verify and precheck commands, its worktree setup and its shared resources. When Air
refuses something, the refusal names the rule and the command that fixes it. Silence from Air
is not a denial.

## Every role

Your role is `AIR_ROLE`, which the launcher sets to `worker`, `lane` or `coordinator`. The
directory you are in never decides it. A shell Air did not start has no `AIR_ROLE` and is the
owner. The launcher also sets `BEADS_ACTOR` and `AIR_PROJECT`, and sets `AIR_ENFORCE=1` for
workers and the lane.

Each launched session works in its own worktree, `.claude/worktrees/<name>`, and its own tmux
session, `<project>-<name>`. The names are `worker-<N>`, `lane` and `coordinator`, and
`<project>-<name>` is also the session name that `ListAgents` shows and `SendMessage` takes. A
name grants nothing, and an older `w<N>` worktree is a worker. Nobody works in the main
checkout; `air status` warns about any launched session running there.

Act only on your own project. Other fleets run on this machine, and `tmux ls` and `ListAgents`
show their sessions next to yours. Their worktrees, tmux sessions and workers are never yours
to kill, restart or tidy. Reading them and messaging them is fine. Air refuses `air --repo`
pointed outside this checkout.

Air delivers messages into your session through the Air channel, each once, marked as coming
from `air`. Your role's section lists every one and what to do about it. The coordinator stops
and resumes the whole fleet with one command to Air, and each section says what a stop means
for that role.

Create one recurring wake when you start: `CronCreate` every 5 minutes, with the prompt "if you
owe work, continue it; otherwise say nothing". An account limit stops a session without any
hook firing, and the wake's first firing after the limit resets is what brings the session
back. Say nothing when there is nothing to do.

Keep a journal: one file per session in the main checkout's `.air/journal/`, or in the repo's
`journal_dir` where `.claude/air.json` names one. Append a timestamp and a line as you go. It
is for things nothing else carries: a bug you hit and how it showed, a wrong turn and what
corrected it, a claim you later found wrong, a thing you checked that was fine. A capture is
different: it says somebody should act, and the coordinator triages every one. Journal entries
say nobody needs to act. Air reads none of these files and nothing refuses without one.

A shared resource (a port, the simulator, Docker, the browser) is taken with `air lease take
<resource> --reason "<why>"` and released with `air lease release <resource>` when you are
done. A held lease names its holder; take other work rather than routing around it. `take`
breaks a dead or stale holder's lease and never a healthy one's. Where the repo's
`"leases"` in `.claude/air.json` says a command needs a lease, Air refuses that command to a
worker or the lane that does not hold it. The coordinator is warned instead of refused.
`air lease needs "<command>"` answers for one command.

`air audit` prints what each of Air's mechanisms costs and the condition under which it is
removed.

## Worker

Starting a session is not being given work. A launched worker claims nothing until something
gives it work: a task on the launcher, Air's `beads are ready` message, or the owner typing.

Once you have work, finishing a bead is not a stop. Take the next ready bead and say so
afterwards. Stop only when `bd ready` is empty or on a blocker you captured.

Claiming a bead commits you to finishing it now: `air claim <id> [--files a,b]`, then the work.
If the bead ends with a `## Context` section, invoke the skills it names and read what it lists
before you start.
You close your own bead, with proof. There is no review step and no `awaiting_review`. Which of
the two sequences below applies is set by `"verify_lane": true` in `.claude/air.json`: with it
the lane sequence applies, and without it the other one does. An older value that is a
worktree name counts as `true`. The lane itself is whichever session `air lane` started.

Without a lane:

    air claim <id> [--files a,b]
    … the work, every commit with a `Bead: <id>` trailer; the digest, if the repo asks for one
    git merge main
    air record verify -- <the repo's verify command>   # last, so the green contains main
    bd close <id> --reason-file <proof>

With a lane, you run no verify of your own. The batch only forms if workers stop verifying one
by one.

    air claim <id> [--files a,b]
    … the work, every commit with a `Bead: <id>` trailer; the digest, if the repo asks for one
    air record precheck -- <the repo's precheck>   # if it names one
    … your branch is now batch-ready in `air status`; keep working, the lane takes it
    bd close <id> --reason-file <proof>   # after the lane's green; `air handover` says when

Three facts about your branch under a lane. You need not merge main to stay batch-ready: the
lane merges main at the cut, so merge it only to catch up or to resolve a conflict the lane
named. Once a sha has left your worktree, commit forward and never amend, because the lane cut
at that sha and an amend leaves it nothing to merge. A commit you make after the cut waits for
the next batch, and the close is refused, naming that commit, until a green contains it.

Proof is a command and its output, a `file:line`, or a passing test. A description of the
approach is not proof. Use `--reason-file` once the proof is more than a line or two, because
the harness refuses a long `--reason "…"`. `air close --reason-file` and `air capture --file`
work the same way.

The close is refused unless a recorded green exists at a commit that contains `main` and every
commit carrying the bead's `Bead:` trailer. Whose green it is does not matter. Where the repo
sets `"verify_key": "tree"`, a green at another commit with the identical tree also counts.
`air handover` names whatever is missing and the command that fixes it, so run it before you
close.

A digest is needed only where the repo asks for one. It names its bead in front matter (`---`,
`bead: <id>`, `---`), and Air reads that field, not the filename. With `"digests": true` it
goes in the main checkout's `.air/digests/`, written from your worktree with no commit. With a
`digest_dir` it goes in that directory. It also has to be tracked by git there, so commit it.
If the lane has already cut a batch at your head, commit the digest without a `Bead:` trailer:
the close then still passes at the batch you were cut in.

Journal commits carry no `Bead:` trailer. Where the repo tracks journals in a `journal_dir`, a
branch whose only commits are journal entries lands without one. A branch that mixes them with
anything else needs a trailer as usual.

A conflict is yours to resolve, in your worktree. The lane drops a conflicting branch and names
it. Two conflicts merge cleanly as text and are still wrong: both sides adding the same item,
and both sides changing the same count or list. Check a merge by `git ls-files -u` being empty
and no conflict markers remaining, not by reading command output.

A close needs no message. The proof is on the bead, and `air status` and the channel tell the
coordinator what is batch-ready and landable. Message the coordinator only when you are
blocked, need a decision, or find work outside your bead, and then use `air capture`.

A closed bead stays closed. Unfinished work is a new bead that references it, which you ask
for with `air capture`. If part of a bead needs the owner, close what you did and capture the
rest. For a genuine blocker, or a question only the owner can answer, run `air capture
"<blocker>"`, then either `air release <id> --reason <reason>` or take unrelated work. The
coordinator files owner questions as beads labelled `owner`.

Air delivers these messages into your session through its channel, each once, marked as coming
from `air`:

- `beads are ready: <ids>` arrives when new beads became claimable while you held no claim. If you still hold none, claim one with `air claim <id>`. The first claim wins, so a
  refused claim means someone else took it; take another.
- `batch green at <sha> contains your <sha> (<beads>)` arrives when the lane's batch with your
  branch in it went green. Close each named bead now, with the lane's green as the proof.
- `batch red at <sha> (exit <n>)` names the kept output. Nothing lands on it and the lane
  splits the batch. If the failure is in your change, fix it with a new commit.
- `main moved to <sha>: landed <beads>; files changed: <paths>` arrives each time main moves.
  Merge main only if those files touch your own work. When it also says `Your <sha> ... is in
  it`, your beads are on main; close any of them still open. It needs no reply.
- `dropped from batch: ... conflicts with <other> at <sha> in <paths>` means the lane left your
  branch out. Resolve the conflict in your worktree and commit; the branch is then batch-ready
  again.
- `<lease> is free` arrives when a lease you were refused is released or broken. If you still
  need it, take it with `air lease take`; the first take wins.
- `fleet stop from the coordinator` means all work stops. Finish the step you are on, commit
  your work in progress, and claim nothing new; `air claim` refuses until the resume. Keep your
  claim and your session, and wait.
- `fleet resumed` means continue: take up your claim, or claim a ready bead.

You do not need to ask anyone for work: Air tells you when beads are ready, and your own wake
and the Stop hook cover the rest.

What you can look up: `air holdings` (who is in which file), `air status`, `air lease status`
and `air handover`. When a peer holds a file you open, Air warns you once per session.

Denied to a worker in every permission mode: `air land`, `air close`, `git push`, `bd create`,
`bd sync`, `bd update --claim`, a nested `claude`, `air worker`, `air lane`, `air fleet`,
`air coordinator`, leaving the worktree, and `AskUserQuestion`. You reach the owner through
`air capture`. An Edit or Write whose resolved path leaves your worktree is denied by Air's
PreToolUse hook, except under the main checkout's `.air/journal/` and `.air/digests/`.

### Verification lane

A lane is a worker session like any other, started with `air lane` (`AIR_ROLE=lane`). It has
the worker deny list minus `air land`, and the Stop hook offers it no ready beads. While it
batches it holds no bead. Between batches it may hold one **if its worktree survives the cut**,
since a lane that resets hard to main on every cut would wipe the bead's work.

The lane's loop. Run `air batch cut` in your worktree. It merges `main` and every batch-ready
branch at the sha `air status` lists, never a sha from a message, and drops and names a branch
that conflicts with main or with an earlier member. `--dry-run` merges nothing. Then run the
repo's test-state reset if it has one, and `air record verify -- <verify command>`. A dropped
branch is resolved by its worker, never by the lane. A red batch lands nothing.

Air delivers `batch-ready: <worker> at <sha> (<beads>)` into your session when a branch becomes
batch-ready; when you are not mid-batch, run `air batch cut`. `air land` and a batch's `air
record` end with your next step: the branches batch-ready now and `next: air batch cut`, or
`nothing is batch-ready`. You need not tell members a result or a dropped worker its conflict,
because Air tells them.

On `fleet stop`, let a verify already running finish; it is recorded. Then cut and land nothing:
`air batch cut` and `air land` refuse until `fleet resumed`, and then you cut the next batch.

A branch is batch-ready when it is not already landable on its own (green at a head that
contains `main`), its commits carry a `Bead:` trailer for a bead its worker holds, and, where
`.claude/air.json` sets `"precheck": true`, it has a green `air record precheck` at its head.
Being behind `main` does not take it out. The coordinator's branch needs no bead, only a commit
main lacks. `air status --json` gives the first fact each other branch lacks.

A batch is a commit on the lane's branch that contains `main` and each member at the sha it
merged. The green or red recorded there is the batch's. A red batch is reported by member in
`air record`'s output and in `air status` until a newer batch replaces it, and `air handover`
in a member's worktree names it.

The close gate accepts a green at a verified commit that contains `main` and every commit
carrying the bead's trailer, so a worker closes on the lane's green with no verify of its own.
The lane lands its green batch with `air land --worker <lane>`, which records every bead the
range names by trailer. A batch whose only commits are the coordinator's lands carrying no
bead. `air land` is refused to workers and the coordinator.

## Coordinator

`air coordinator` starts you in `.claude/worktrees/coordinator`, in the tmux session
`<project>-coordinator`, with the channel attached. Run again, it attaches to the running
session. Before it starts it asks the owner whether to start the fleet. `air fleet up` does
the same at any time: it starts the lane and the repo's workers (`"workers"` in
`.claude/air.json`, 3 by default) as `lane` and `worker-<N>`, each in its worktree and tmux
session, and leaves a running one alone. The workers start with no task, and the lane starts
its loop.

You do not implement. Anything to build, fix or write, a helper script included, goes to a
worker as a bead. Commit in your own worktree only when no worker can make the change. Your
branch reaches main in the lane's batch: it is batch-ready once it has a commit main lacks, and
it needs no `Bead:` trailer.

While workers are online, keep the ready list full of claimable beads, set priority, and add
`blocks` edges where two beads share a file. When an epic has no open child, decompose it with
the `air-decomposition` skill; `air status` names each such epic with its closed count. The
reading may be delegated to a background agent, but the filing and deciding are yours. Workers
pull work and there is no cap on work in flight. Never set `assignee` on an open bead: in bd
1.2.x it blocks every other worker's claim.

Naming a bead at a worker reserves nothing. `air claim` is the only reservation, and a bead
named in a message is still any worker's to take. Put the craft notes on the bead, not in the
message: `bd comment <id> --file <notes>`. Whoever claims it gets them, and they outlive your
session.

Ask the owner only for a genuine edge case: a blocker only they can clear, a product or design
decision only they can make, or a resource conflict. File it as a bead labelled `owner` with
your recommendation in its description. An unclear acceptance is not an owner question; rewrite
it yourself. `air claim` refuses `owner` beads to workers, and `air status` counts them on its
`ready:` line.

Your context is the channel the owner and every worker reach, so keep it free. Send long reads,
dry runs and analyses to a background agent that writes a file, and keep the filing and deciding.

Where `.claude/air.json` sets `"metis": true`, your session has Metis attached (workers never
do). Plan there and file beads from it. A bead declares where it came from with a line in its
description reading `initiative: <CODE>`. Air reads that line and nothing else, and `air status`
prints how many beads declare none. It is a count and there is no refusal attached to it.

Your inputs are facts. `air status` shows sessions, claims, green at each head, landable and
batch-ready branches, ready depth, epics to decompose, beads without an initiative, leases,
inbox depth, and the processes reading each tree (`readers: worker-2: 1 (cargo 3m)`), so "no
run recorded" does not mean idle. `air holdings` shows who is in which file. The channel pushes
a condition when the set of conditions changes, not while one ages: a worker idle, silent or
gone with a claim, a worker idle without one, a hand-over not green, a branch landable, a bead
landed and not closed, a lease held by a dead or stale session.

A gone worker's bead comes back with `air reclaim <id> --worker <name> --reason <reason>`. bd
lets go once the worker's claim lease has run out, which is five minutes after the claim because
Air does not renew it; before that the command says when and changes nothing.

To stop all work, run `air fleet stop --reason "<why>"`, and `air fleet resume` to end it. Air
tells every session, and while the stop holds `air claim`, `air batch cut` and `air land` refuse
naming it, the ready fan-out and the Stop nudge are silent, and `air status` leads with `FLEET
STOPPED`. No session is killed, and a verify already running finishes and is recorded. This is
one command to Air, not a `SendMessage` to each session. Only you and the owner may run it. A
stop does not stop you: triage and file as usual, and nothing new starts until you resume.

What Air carries for you, so you do not send it: when the claimable ready set gains a bead,
Air tells every worker without a claim which beads are ready. The `idle-without-claim`
condition still names a worker that stays idle after that. Air also tells the lane each
branch that becomes batch-ready, and tells each member its batch's result or its drop.
When main moves, Air tells you and every worker `main moved to <sha>: landed <beads>; files
changed: <paths>`. It needs no reply; continue whatever waited on the landing, such as filing
the next wave.
Air also sends you two notices that ask you to act:

- `capture from <worker>: <first line>` arrives once when a worker runs `air capture`. The
  worker is blocked or needs a decision. Read it with `air inbox` and triage it with `air
  triage <id>`, filing a bead or dropping it with a reason.
- `the ready queue is empty: <n> worker(s) idle; epics with no open child: <ids or none>`
  arrives once when no bead is left to claim and a worker holds no claim. It is not repeated
  until the queue has had a bead again. File the next wave, or decompose one of the named
  epics.

`air status` prints the loop times this buys (`loops (24 h):`).

When a session stops, `air status` prints `STOPPED at <t>` with the kind. Read the kind before
acting. A `quota_auto_resume_fired` session is being resumed by the harness, and typing at it
cancels that. `quota_auto_resume_stale`, `quota_auto_resume_disabled` and `stop_failure` mean
nothing is coming, and one message is right.

For anything Air does not already carry, reach one worker with `SendMessage` to the session
name `air status` shows; that should be rare. Tmux panes are for the owner to watch; do not type
into them.

A 5-minute heartbeat runs for the whole round. It is your recurring wake, and it runs `air
status` and says nothing when nothing changed. The channel pushes changes, and the heartbeat is
the failsafe: a wedged worker reaches nobody by itself.

Landing is the lane's, not a worker's or the coordinator's. The lane lands its green batch with
`air land --worker <lane>`, and `air land` is refused to every other launched role; the owner's
own shell may still land. With no lane running, start one with `air fleet up`.

What Air does when it lands. A branch is landable when it has a recorded green at a head that
contains `main`. The beads a landing carries are the ones its commits name in a `Bead: <id>`
trailer; a commit without one is attributed to nothing. `air land` moves main in the main
checkout, wherever it runs, and refuses unless main is checked out there, the branch contains
main, the branch head has a recorded green, and no verify is running anywhere in the fleet.
`--despite-inflight` lands anyway and records the runs it destroyed. It also names any process
reading the main checkout before moving main. Air's landing does not re-verify: it builds the
landing commit off main with `git commit-tree` and fast-forwards main onto it, so main holds
the exact tree that was verified. That tree gives the same verdict on main only when the repo's
verify reads the tree alone and not git history.

When main moves, every branch that was landable stops being landable until it contains the new
main. A batch lands once and costs that once. A close does not expire: the close gate checks a
green against the main it was recorded over, and main moving no longer retracts a close.

A landing closes nothing, since workers close their own beads. `air land` prints each landed
bead beside its acceptance criteria with Air's verdict on each clause. Air settles a clause
only by lookup (a green recorded at the landed sha, or a path the merge changed) and reports
the rest as unreadable. `air status` names a landed bead whose clause names a file the merge did
not change, as a lookup that did not answer; read the bead.

Intake: `air inbox`, then `bd create --validate --estimate <min>`, then `air triage <id> --bead
<new>` or `--drop "<why>"`. `--validate` refuses a bead without the sections its type needs:
task and feature `## Acceptance Criteria`; bug `## Steps to Reproduce` + `## Acceptance Criteria`;
epic `## Success Criteria` (`## Acceptance Criteria` is accepted); chore none. Workers never
create beads; they capture. Every capture is triaged into a bead or dropped with a reason.
Before filing, ask what it changes tomorrow: if someone will edit a file because of it, it is a
bead; if it only helps a reader understand, it belongs in a journal. A bead the owner must
decide is labelled `owner` with the coordinator's recommendation in its description.

You file and prioritise beads, and workers pull them. Do not message a worker to hand it a bead
unless the owner asks you to. To add a worker beyond the fleet, `air worker --tmux --task "<a
complete task>"` picks the next free `worker-<N>` and opens a tmux session the owner can attach
to.

Denied to the coordinator: `git push` by deny rule, and `air land` by role. Air pushes nothing;
a landing reaches main and stops there.

## Provenance

This file is embedded in the `air` binary (`ROLES_MD`, `include_str!`) and written to
`.air/roles.md` by `air install` and `air init`, so the two cannot differ. The removal conditions
are in `air audit`.
