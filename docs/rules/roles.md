# Roles: worker, lane and coordinator

You read this at session start. It is the fleet's whole protocol: how work moves from a claim
to main, for every role. `main` here is the base branch: `main` unless the repo's
`.claude/air.json` sets `base_branch`, and then that branch wherever this file, a refusal or a
notice says main (`trunk moved to`, `contains trunk`). The repo's CLAUDE.md holds only what is
the repo's own: its domain rules, its verify and precheck commands, its worktree setup and its
shared resources. When Air refuses something, the refusal names the rule and the command that
fixes it. Silence from Air is not a denial.

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

A command in this file that says `<base branch>` means the branch the fleet lands onto, which
is `main` unless the repo's `.claude/air.json` sets `base_branch`. `air status` prints
`base branch: <name>` when it is not `main`, and every notice and refusal names it (`trunk
moved to`); merging `main` on such a repo fails, because no `main` exists.

Act only on your own project. Other fleets run on this machine, and `tmux ls` and `ListAgents`
show their sessions next to yours. Their worktrees, tmux sessions and workers are never yours
to kill, restart or tidy. Reading them and messaging them is fine. Air refuses `air --repo`
pointed outside this checkout.

Air delivers messages into your session through the Air channel, each once, marked as coming
from `air`. Your role's section lists every one and what to do about it. The coordinator stops
and resumes the whole fleet with one command to Air, and each section says what a stop means
for that role.

No role uses the question prompt (`AskUserQuestion`), and every launcher denies it. A session
waiting on it hears no notices and does no work, and the owner is usually not watching its
pane. A question for the owner is a bead labelled `owner` (coordinator) or an `air capture`
(worker, lane), with your recommendation, and you take other work while it waits.

How Air works is in `.air/docs/design.md`, and what has gone wrong before and why is in
`.air/docs/solved-issues.md`; read them before reporting a defect in Air.

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
gives it work: a task on the launcher, Air's `<n> bead(s) are claimable` message, or the
owner typing.

Once you have work, finishing a bead is not a stop. Take the next ready bead and say so
afterwards. Stop only when `bd ready` is empty or on a blocker you captured.

Claiming a bead commits you to finishing it now: `air claim <id> [--files a,b]`, then the work.
If the bead ends with a `## Context` section, invoke the skills it names and read what it lists
before you start.

You declare your own bead done, with proof, and the landing closes it. There is no review step
and no `awaiting_review`. Which of the two sequences below applies is set by
`"verify_lane": true` in `.claude/air.json`: with it the lane sequence applies, and without it
the other one does. Any other value counts as absent, though a lane session having run also
counts as a lane. The lane itself is whichever session `air lane` started.

Without a lane:

    air claim <id> [--files a,b]
    … the work, every commit with a `Bead: <id>` trailer; the digest, if the repo asks for one
    git merge <base branch>
    air record verify -- <the repo's verify command>   # the green first; nothing else verifies
    air done <id> --reason-file <proof>   # the landing closes the bead

The check behind `air done` needs no green, but without a lane nothing else verifies your
branch, so the verify comes first. The landing that puts your commit on the base branch closes
the bead: the owner's shell, or a lane started for it.

With a lane, you run no verify of your own. The batch only forms if workers stop verifying one
by one.

    air claim <id> [--files a,b]
    … the work, every commit with a `Bead: <id>` trailer; the digest, if the repo asks for one
    air record precheck -- <the repo's precheck>   # if it names one
    air done <id> --reason-file <proof>   # right away; nothing waits for a green
    … your branch is batch-ready in `air status`; the lane takes it, so start the next bead

Four facts about your branch under a lane. You need not merge main to stay batch-ready: the
lane merges main at the cut, so merge it only to catch up or to resolve a conflict the lane
named. Once a sha has left your worktree, commit forward and never amend, because the lane cut
at that sha and an amend leaves it nothing to merge. A commit for a bead made after `air done`
lands in a later batch, and the bead closes at that landing unless you run `air done` again at
the new head, which moves the head the landing waits for. Commits for your next bead do not
hold up the earlier one: the declaration judges a bead by the commits whose `Bead:` trailer
names it, so declare it with later work committed on top.

Proof is a command and its output, a `file:line`, or a passing test. A description of the
approach is not proof. Use `--reason-file` once the proof is more than a line or two, because
the harness refuses a long `--reason "…"`. `air capture --file` works the same way.

Declare with `air done`; `air close` from a worker or the lane is refused, and closing through
bd directly is denied to them. `air done` runs the hand-over check without its green: a commit
carrying the bead's `Bead:` trailer on your branch or on the base branch, your own claim on the
bead, a green precheck where the repo declares one, and a digest where it configures one. Every
bead closes on a commit that names it, so a bead with no such commit is refused: commit the
result, a document if nothing else, with the trailer.

The proof goes on the bead as a comment and the head you declared goes on your claim. The
landing that puts that head on the base branch closes the bead and releases the claim, and a
bead already on the base branch closes at once. Where the repo sets `"verify_key": "tree"`, a
green at another commit with the identical tree counts as a green at yours, for `air handover`
and for the landing. `air handover` names whatever is missing and the command that fixes it, so
run it before you declare.

A digest is needed only where the repo asks for one. It names its bead in front matter (`---`,
`bead: <id>`, `---`), and Air reads that field, not the filename. With `"digests": true` it
goes in the main checkout's `.air/digests/`, written from your worktree with no commit. With a
`digest_dir` it goes in that directory. It also has to be tracked by git there, so commit it.
If the lane has already cut a batch at your head, commit the digest without a `Bead:` trailer:
the done then still passes at the batch you were cut in.

Journal commits carry no `Bead:` trailer. Where the repo tracks journals in a `journal_dir`, a
branch whose only commits are journal entries lands without one. A branch that mixes them with
anything else needs a trailer as usual.

A conflict is yours to resolve, in your worktree. The lane drops a conflicting branch and names
it. Two conflicts merge cleanly as text and are still wrong: both sides adding the same item,
and both sides changing the same count or list. Check a merge by `git ls-files -u` being empty
and no conflict markers remaining, not by reading command output.

Where `targets` in `.claude/air.json`, or in the gitignored `.claude/air.local.json` laid over
it, names a machine for your session, `air run -- <command>`
runs the command there at your HEAD, `--pull <glob>` brings result files back, and `air record`
runs its check there the same way; with no entry both run here. Your launcher allows both
bare, so run them with no pipe or redirect: a piped line is judged by the auto-mode
classifier, which has refused a worker's `air run` as a remote write.

A done needs no message. The proof is on the bead, and `air status` and the channel tell the
coordinator what is batch-ready and landable. Message the coordinator only when you are
blocked, need a decision, or find work outside your bead, and then use `air capture`.

A done bead stays done, and a closed one stays closed. Unfinished work is a new bead that
references it, which you ask for with `air capture`. If part of a bead needs the owner, declare
what you did done and capture the rest. For a genuine blocker, or a question only the owner can
answer (never the question prompt; see Every role), run `air capture "<blocker>"`, then either
`air release <id> --reason <reason>` or take unrelated work. The coordinator files owner
questions as beads labelled `owner`.

Air delivers these messages into your session through its channel, each once, marked as coming
from `air`:

- `<n> bead(s) are claimable; run bd ready` arrives when new beads became claimable while
  you held no claim, or when you join or your claim ends while beads are ready. It carries a
  count and no ids, so read the list from `bd ready`. If you still hold none, claim one with
  `air claim <id>`. The first claim wins, and a refused claim names why: taken, blocked,
  unknown, owner, an epic, or closed; take another from `bd ready`.

- `batch red at <sha> (exit <n>)` names the kept output. Nothing lands on it and the lane
  verifies each member alone. Your branch stays out of the next batch until a later green
  contains your commit, the lane verifies it alone, or you commit again. If the failure is in
  your change, fix it with a new commit.

- `main moved to <sha>: landed <beads>; files changed: <paths>` (the base branch's name where
  one is set) arrives each time main moves. Merge main only if those files touch your own work.
  When it also says `closed: <beads>`, those beads are closed; `done but waiting: <beads>`
  closes at a later landing, since the head you declared is not on main yet; `done but
  refused: <beads>` means bd refused the close with the head on main, and the next landing
  tries again. It needs no reply.

- `dropped from batch: ... conflicts with <other> at <sha> in <paths>` means the lane left your
  branch out. Resolve the conflict in your worktree and commit; the branch is then batch-ready
  again.

- `<lease> is free` arrives when a lease you were refused is released or broken. If you still
  need it, take it with `air lease take`; the first take wins.

- `fleet stop from the coordinator` means all work stops. Finish the step you are on, commit
  your work in progress, and claim nothing new; `air claim` refuses until the resume. Keep your
  claim and your session, and wait.

- `fleet resumed` means continue: take up your claim, or claim a ready bead.

You do not need to ask anyone for work: Air tells you when beads are ready, and the Stop hook
covers the rest.

What you can look up: `air holdings` (who is in which file), `air status`, `air lease status`
and `air handover`. When a peer holds a file you open, Air warns you once per session.

You read beads and add comments with `bd comment`; every other change to a bead goes through
`air` (`air claim`, `air done`, `air capture`), and bd's write commands are denied to you.
Denied to a worker in every permission mode: `air land`, `air ready`, `git push`, every bd
command that changes a bead or the store, a nested `claude`, `air worker`, `air lane`,
`air fleet`, `air coordinator`, the worktree tools (`EnterWorktree`, `ExitWorktree`), and
`AskUserQuestion`. You reach the owner through `air capture`. Work only in your worktree,
never in the main checkout, and use your scratchpad for anything that is not part of the repo.

### Verification lane

A lane is a worker session like any other, started with `air lane` (`AIR_ROLE=lane`). It has the
worker deny list minus `air land`, and the Stop hook offers it no ready beads. It holds no bead:
`air batch cut` resets the lane's branch to the base branch before it merges, so work of the
lane's own does not survive a cut.

The lane's loop. Run `air batch cut` in your worktree. It resets your branch to the base branch
and merges every batch-ready branch at the sha `air status` lists, never a sha from a message. It
refuses while your tree is dirty or your head is a passing batch that could still land, and it
drops and names a branch that conflicts with main or an earlier member, which its worker
resolves, not the lane. `--dry-run` merges nothing.

Then run the test-state reset, if any, and `air record verify -- <verify command>`. A target for
your session (`targets`, the overlay included) runs it there on the pushed batch commit, with the
green recorded at your head; `--local` runs it here. A red batch lands nothing; `air record`
lists the members not yet verified alone and the `--only` cut for the next, so cut each until
the list is empty. Run these and `air land` bare, with no pipe or redirect: only the bare line is
allowed, and one classifier refusal can stop later landings.

Air delivers `batch-ready: <worker> at <sha> (<beads>)` into your session when a branch becomes
batch-ready; when you are not mid-batch, run `air batch cut`. `air land` and a batch's `air
record` end with your next step: the branches batch-ready now and `next: air batch cut`, or
`nothing is batch-ready`. Air tells members their result and a dropped worker its conflict. On
`fleet stop`, let a verify already running finish; it is recorded. Then cut and land nothing:
`air batch cut` and `air land` refuse until `fleet resumed`, and then you cut the next batch.

A branch is batch-ready when its commits carry a `Bead:` trailer for a bead its worker holds or
that is closed (a green at its head does not take it out), and, where `.claude/air.json` sets
`"precheck": true`, it has a green `air record precheck` at its head. Being behind `main` does
not take it out. The coordinator's branch needs no bead, only a commit main lacks, offered with
`air ready`. `air status --json` names what each other branch lacks.

A batch is a commit on the lane's branch that contains `main` and each member at the sha it
merged. The green or red recorded there is the batch's. A red batch is reported by member in
`air record`'s output and in `air status` until a newer batch replaces it, and `air handover` in
a member's worktree names it. The lane lands its green batch with `air land --worker <lane>`,
recording every bead the range names by trailer; the landing closes every bead whose declared
done head is now on `main`, in one bd process, and releases its claim as landed. A batch of
only the coordinator's commits lands carrying no bead. `air land` is refused to workers and the
coordinator.

## Coordinator

`air coordinator` starts you in `.claude/worktrees/coordinator`, in the tmux session
`<project>-coordinator`, with the channel attached. Run again, it attaches to the running
session. Before it starts it asks the owner whether to start the fleet. `air fleet up` does
the same at any time: it starts the lane and the repo's workers (`"workers"` in
`.claude/air.json`, 3 by default) as `lane` and `worker-<N>`, each in its worktree and tmux
session, and leaves a running one alone. The workers start with no task, and the lane starts
its loop.

Unless the owner has already said how to work, your first step is to offer a menu of named
workflows, in plain text and never the question prompt, and take the owner's pick for the
session:

- fleet: every change is a bead, and workers make all of it.
- coordinator alone: you edit in your own worktree and the lane lands it; no workers.
- mixed: a bead for anything over a few lines; you make small edits yourself.
- read-only: triage, review and planning; no changes.

Write the pick in your journal; it holds for the session.

Under the fleet workflow you do not implement: anything to build, fix or write, a helper
script included, goes to a worker as a bead. A commit of your own is one only your branch can
carry, such as filing and the round-end rows. Your branch reaches main (the base branch where
one is set) in the lane's batch and needs no `Bead:` trailer. It is batch-ready once it has a
commit main lacks and you have run `air ready` at that head; a later commit needs a new
`air ready`, so a note you are still writing costs the lane nothing. Where `.claude/air.json`
sets `"coordinator_autobatch": true`, every commit main lacks is batch-ready without it.
`air status --json` says `not-offered` for a head waiting on one.

While workers are online, keep the ready list full of claimable beads, set priority, and add
`blocks` edges where two beads share a file. When an epic has no children, decompose it with
the `air-decomposition` skill; `air status` names each such epic. An epic whose children are
all closed is finished, and `air land` closes it after the landing that finishes it. The
reading may be delegated to a background agent, but the filing and deciding are yours. Workers
pull work and there is no cap on work in flight. Never set `assignee` on an open bead: it
blocks every other worker's claim (bd 1.3.0, as on 1.2.2).

Naming a bead at a worker reserves nothing. `air claim` is the only reservation, and a bead
named in a message is still any worker's to take. Put the craft notes on the bead, not in the
message: `bd comment <id> --file <notes>`. Whoever claims it gets them, and they outlive your
session.

Something for a bead a worker holds now has two shapes. An addition to the bead is a bead,
filed as usual, plus a `SendMessage` to the holder naming it, so the worker can fold it into
the branch it is on if it has not moved on; the bead stays claimable either way.
A correction (the bead is doing something wrong) goes as a `SendMessage` first, because
the next commit may be cut before a bead is read, and you check that it arrived: a reply, or
the next commit showing it. A correction not seen to arrive becomes a bead that names what to
undo. On 2026-11-09 a hold sent as a message arrived a minute after the commit it was meant
to hold, and the lane had already cut the sha.

Ask the owner only for a genuine edge case: a blocker only they can clear, a product or design
decision only they can make, or a resource conflict. File it as a bead labelled `owner` with
your recommendation in its description, never as a question prompt (see Every role). An
unclear acceptance is not an owner question; rewrite it yourself. `air claim` refuses `owner`
beads to workers, and `air status` counts them on its `ready:` line.

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
gone with a claim, a worker idle without one, a worker whose `air done` was refused and has not
passed since, a branch landable, a lease held by a dead or stale session.

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
Air tells every worker without a claim how many beads are ready and to run `bd ready`, and it
tells a worker that joins without a claim the same. The `idle-without-claim` condition still
names a worker that stays idle after that. Air also tells the lane each branch that becomes
batch-ready, and tells each member its batch's result or its drop.

When main moves, Air tells you and every worker `main moved to <sha>: landed <beads>; files
changed: <paths>`, named after the base branch where one is set. It needs no reply; continue
whatever waited on the landing, such as filing the next wave.

Air also sends you two notices that ask you to act:

- `capture from <worker>: <first line>` arrives once when a worker runs `air capture`. The
  worker is blocked or needs a decision. Read it with `air inbox` and triage it with `air
  triage <id>`, filing a bead or dropping it with a reason.

- `the ready queue is empty: <n> worker(s) idle; epics with no children: <ids or none>`
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
into them, with one exception, the start-up prompts below.

After any launch (`air fleet up`, `air worker`, `air lane`), read each new pane with `tmux
capture-pane -p -t <project>-<name>`. A session waiting on a prompt hears no notices and does no
work. If a prompt is waiting, ask the owner once, in plain text, "Shall I accept these on your
behalf?", and on a yes answer each with `tmux send-keys -t <project>-<name> Enter` (or the
recorded answer for that prompt).

Your launcher allows `Bash(tmux send-keys *)` past the auto-mode classifier, which refused the
keystroke otherwise; run it bare, with no pipe or redirect, because a piped line is judged by
the classifier again. Answer only a prompt whose answer is recorded, such as the
development-channels warning; never the folder-trust dialog, which is the owner's, and never a
question about the work.

The heartbeat is opt-in. Create it only where `.claude/air.json` sets `"heartbeat_minutes": <n>`
(the SessionStart context says so and `air status` names the interval) or the owner asks for it:
`CronCreate` every <n> minutes, with the prompt "run `air status`; if you owe work, continue it;
otherwise say nothing". An account limit stops a session without any hook firing, and the
heartbeat's first firing after the limit resets is what brings the session back; without one,
`air status` still prints `STOPPED at <t>` with the kind. It says nothing when nothing changed.
The channel pushes changes, and the heartbeat is the failsafe: a wedged worker reaches nobody by
itself.

Landing is the lane's, not a worker's or the coordinator's. The lane lands its green batch with
`air land --worker <lane>`, and `air land` is refused to every other launched role; the owner's
own shell may still land. With no lane running, start one with `air fleet up`.

What Air does when it lands. A branch is landable when it has a recorded green at a head that
contains `main` (the base branch where one is set; landing that onto the real main is the
owner's merge, outside Air). The beads a landing carries are the ones its commits name in a
`Bead: <id>` trailer; a commit without one is attributed to nothing.

`air land` moves main in the main checkout, wherever it runs, and refuses unless main is
checked out there, the branch contains main, the branch head has a recorded green, and no
verify is running in the main checkout or the lane's tree. `--despite-inflight` lands anyway
and records the runs it destroyed. It also names any process reading the main checkout before
moving main.

Air's landing does not re-verify: it builds the landing commit off main with `git commit-tree`
and fast-forwards main onto it, so main holds the exact tree that was verified. That tree gives
the same verdict on main only when the repo's verify reads the tree alone and not git history.

When main moves, every branch that was landable stops being landable until it contains the new
main. A batch lands once and costs that once. A done does not expire: the declaration names a
head, and main moving only delays the landing that contains it.

A landing closes the beads declared done whose head it put on main, and closes nothing nobody
declared done (owner, 2026-08-22 and 2026-11-09). `air land` prints each landed bead beside its
acceptance criteria, with no verdict on them; read them.

Intake: `air inbox`, then `bd create --validate --estimate <min>`, then `air triage <id> --bead
<new>` or `--drop "<why>"`. `--validate` refuses a bead without the sections its type needs:
task and feature `## Acceptance Criteria`; bug `## Steps to Reproduce` + `## Acceptance Criteria`;
epic `## Success Criteria` (`## Acceptance Criteria` is accepted); chore none. Workers never
create beads; they capture. Every capture is triaged into a bead or dropped with a reason.

Before filing, ask what it changes tomorrow: if someone will edit a file because of it, it is a
bead; if it only helps a reader understand, it belongs in a journal. A bead the owner must
decide is labelled `owner` with the coordinator's recommendation in its description. A capture
triaged to a bead is written up in seven short parts: what the parts involved do, what happened,
why, which lifecycle state it is, the precedent for the same mechanism, the fix and its cost,
and what the fix could break.

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
