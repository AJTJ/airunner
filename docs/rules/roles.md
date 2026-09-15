# Roles: coordinator and worker

> Read at session start. Markers: **[Air enforces]** a refusal, deny rule, or native block;
> **[fact]** something Air records or answers; everything else is yours to judge. Domain rules
> AND the repo's own work flow — how a finished bead is handed on — live in the repo's
> CLAUDE.md, not here; Air states what it records and what it refuses. Background:
> [`../research/agent-roles-and-confinement.md`](../research/agent-roles-and-confinement.md).

## Which role am I

Your role is `AIR_ROLE`, set by the launcher: `worker` or `coordinator`. It is never the
directory you are in, since any role may work in a worktree. A shell Air did not start has no
`AIR_ROLE` and is the owner. Air records the role on every session and event. **[fact]**
`AIR_ROLE`, `BEADS_ACTOR` and `AIR_PROJECT` are set by the launcher, never by files.

**Act only on your own project; talk to any of them.** Other fleets run on this machine.
Another project's worktrees, tmux sessions and workers are never yours to kill, restart,
re-model or tidy — `tmux ls` is machine-wide and `ListAgents` lists their sessions next to
yours. Reading them and messaging them is fine and often the point: the cross-project channel
caught three wrong claims on 2026-08-22, one of which both coordinators had backwards. `air
--repo` outside this checkout is refused; messaging is not fenced and will not be (air-3oq).
**[Air enforces: `--repo`]** The tmux half was a refusal too (air-0lk); it fired zero times in
its whole life and was deleted on 2026-08-29 (air-9u6). The rule stands, the machinery does not:
another project's sessions are still not yours to kill, and now nothing but this line says so.

Before choosing a closed default, ask what a wrong denial looks like from the outside. For
`tmux kill-session`, a refusal someone reads. For a message, an empty room nobody notices.
Closed defaults belong to the first kind (air-5re).

## Worker (one per worktree, one bead at a time)

**Starting a session is not being given work.** A launched worker waits: it claims nothing and
takes no bead until something triggers it — a task on the launcher, a message, the owner typing.
Nothing enforces this and nothing needs to: with no `--task` the launcher passes no prompt at
all, and this file arrives via `--append-system-prompt-file`, so an untriggered session never
runs a turn (owner, 2026-08-29, air-7q5). Removed when a launch path exists that starts a turn
without a trigger.

**Once you have work, finishing a bead is not a stop.** At WIP 0 take the next ready bead and say
so afterwards; stop only when `bd ready` is empty or on a blocker you captured. (Two workers read
"next bead" as "wait for review" and idled 20 min, 2026-08-22; the adopter's 51-minute idle of
2026-08-15 was fixed by this sentence and never recurred. Removed never.) The run-on begins at
your first piece of work, not at session start; the two sentences above are the whole of the
difference.

Claiming a bead is a commitment to work it to completion now: `air claim <id> [--files a,b]`,
then the work. **How a finished bead is handed on is the repo's own flow, in its CLAUDE.md,
not Air's to prescribe** — some repos hand over for review, some close with proof.

**Signal the coordinator when you close a bead** (owner, 2026-08-29). One `SendMessage`: the
bead, the proof, and whether your branch is now landable. Nothing polls for this on your behalf
in time to be useful, and the coordinator's next move depends on it. Air records the fact
underneath — the `landable` condition fires once when a branch first goes green with `main`
merged — so a signal you forget is not a fact anyone loses; it is one that arrives later than
it should have. **[fact]**

Two constraints, because Air checks them and they are about ordering rather than procedure:

- **The recorded green has to be at the commit you hand on, and that commit has to contain
  `main`.** So merge `main` first and run `air record verify -- <cmd>` last; a green recorded
  before the merge is a green for a tree nobody will land. Whose green does not matter: a
  green at a sha is a green at that sha whoever ran it (air-7wf). Where the repo declares
  `verify_key: tree` in `.claude/air.json`, a green at another commit with the identical tree
  counts too, which is what makes a fast-forward onto a landing green with no re-verify.
  **[fact]**
- **A digest, where the repo configures `digest_dir`, has to name its bead** in front matter
  (`---` / `bead: <id>` / `---`) and be written with the work rather than after the fact. Air
  reads the declared field, not the filename: a digest for another bead used to satisfy the
  gate (air-agq). **It also has to be tracked by git** (air-ahl): a file only your worktree has
  is a note to self, and the gate exists for the reader who was not there. An adopter's worker
  used an untracked digest to satisfy the gate and reported it against its own interest.
  **If your lane has already cut a batch at your head, commit the digest WITHOUT a `Bead:`
  trailer.** The batch green has to contain every commit that NAMES the bead, and an
  untrailered commit never joins that set, so your head moves and the close still passes at the
  batch you were cut at. This is the one place a commit is deliberately not trailered.

`air handover` names whatever is missing and the command that fixes it, so run it before you
finish rather than guessing which of the two bit you. **[fact]**

A closed bead stays closed; unfinished work is a new bead that references it (ask via
`air capture`). Stop only for a genuine blocker or an owner-only decision; say so in one line
with `air capture "<blocker>"`, then `air release <id> --reason <why>` or take unrelated
work. A question only the owner can answer goes the same way: the coordinator files it as a
bead labelled `owner` (air-uef).

Facts available to you: `air holdings` (who is in which file), `air status`, `air lease status`,
`air handover` (what is missing and the command that fixes it). A warning that a peer holds a
file you are opening arrives once per session. **[fact]**

**Create one recurring wake when you start, and leave it alone.** `CronCreate` a task every
5 minutes whose prompt is "if you owe work, continue it; otherwise say nothing", pointing at
`air status` and your claim rather than restating a queue that will be stale by the time it
fires. It exists because an account limit stops a session without any hook firing, and the
harness only sometimes arms its own auto-continue: on 2026-09-06 five of seven sessions on this
machine armed one and were working within 70 s of the reset, while the two that did not were the
two that stayed down, and the one of those with no wake sat dead for 79 minutes. **A scheduled
task keeps firing while the session is limited** — measured, six fires over 24 minutes — and the
first fire after the reset is the recovery. It cannot pile up: the harness does not catch up
missed fires, so a stopped session accrues one wake, not a stack. Say nothing when there is
nothing to do; a wake that reports is noise 288 times a day. Removed when the harness arms its
own wait for every session it stops, for a whole round (air-1n3). **[fact]**

**Keep your own journal, and put in it the things nothing else will carry.** One file per
session under the repo's `journal_dir` (`.claude/air.json`; `docs/journal` where `air init`
scaffolded it), appended as you go, a timestamp and a line. What belongs: a bug you hit and how
it presented, a wrong turn and what corrected it, a claim you later found was wrong, a thing you
checked that turned out fine.

A journal commit carries no `Bead:` trailer, because it is not work on a bead, and **a branch
whose only commits are journal entries lands without one** (air-kexg). A branch that mixes them
with anything else needs a trailer exactly as before: this is a name for the one commit you
legitimately write that names no bead, not a way round the trailer. Two workers derived the
opposite from correct premises on 2026-09-06 and were told to amend with a bead they had not
touched. **[fact]**

**How it differs from a capture, which is the distinction that matters** (alerts, 2026-09-06):
a capture says somebody should do something, and the coordinator triages every one. These entries
say **nobody** should do anything — they are for whoever is next, not for the queue. Sending them
as captures fills the inbox with things that need no triage; sending them as messages means they
survive only while the recipient's session does. On 2026-09-06 a coordinator hit an account limit
and the round's best material existed only in its memory of messages.

**Nothing gates on it.** Air reads none of these files, nothing refuses without one, and no
condition counts them. **[fact]** Removed when a round log can be written from digests and
captures alone.

Things that need a shared resource (a port, the simulator, Docker, the browser):
`air lease take <resource> --reason "<why>"`; release when done. A held lease names its holder;
do not route around it. **[Air enforces: a healthy holder is not broken by `take`]**

Not available to a worker, by deny rule in every permission mode: `air land`, `air close`,
`git push`, `bd create`, `bd sync`, raw `bd update --claim`, a nested `claude`, leaving the
worktree, and `AskUserQuestion`. The owner is reached through `air capture "<question>"`: the
coordinator files it as a bead labelled `owner`, the queue shows in `air status`, and the
question and its answer leave a row (owner, 2026-08-30; air-bm3). An Edit or Write whose
resolved path leaves your worktree is denied by Air's PreToolUse hook (air-8gj); the harness's
own worktree isolation is off, since in the adopter's record it stopped no observed write to
main and cost 455 refusals in five days, 88% with no git token. **[Air enforces]** The one
refusal: the `bd` write that
ends your work on a bead — `bd close`, or `bd update -s closed` / `-s awaiting_review`,
whichever your repo uses — is denied without a recorded green at HEAD that contains `main`.
Worker launches set `AIR_ENFORCE=1` and the hook names the fixing command. Enforced after the
first bypass of the advisory gate (tty-fix, 2026-08-22 06:00, air-i59); removed when a full
round passes with zero `handover-not-green` events. **[Air enforces]**

### Verification lane (a worker whose work is verifying other branches)

A lane is a worker session like any other: same launcher, same deny list, same claims. While
it batches it holds no bead; between batches it may hold one **if its worktree survives the
cut** (air-80x.6, narrowed by air-4noi). A lane that resets hard to main on every cut — the
simplest way to make a batch contain main and nothing else — would hold that bead in a tree it
is about to wipe; a lane that merges main forward and integrates on a throwaway branch keeps
it. Which of the two a repo runs is its own flow, and Air reads neither. Facts and refusals
only, nothing about cadence or who the lane is: that is the repo's flow, in its CLAUDE.md.

**What Air records.** A batch is a commit on the lane's branch that contains `main` and the
member branches at the shas it merged. `air record verify -- <cmd>` at that commit records the
green or red, and the landing row names the member heads the batch contained (air-80x.2).
`air status` lists the branches that are batch-ready as a fact, `batch-ready: <worker> at
<sha> (<beads>)`: head contains `main`, no green at that head, a `Bead:` trailer naming a bead
the worker holds; `--json` gives the first fact each other branch lacks (air-80x.3). A red
batch is reported by member, in `air record`'s output and in `air status`, until a newer batch
supersedes it; nothing lands, closes or claims differently on a red (air-80x.4). **A member
can look it up rather than wait to be told**: `air handover`, run in your own worktree, names
the batch, the lane and where the lane's output is when the standing red batch has your branch
in it (air-hpp8). Silence there is not a statement that you were not in one — Air knows the
membership only from what the run recorded. **[fact]**

**What Air refuses, and what it accepts.** The close gate accepts a green at a verified commit
that contains `main` and every commit carrying the bead's trailer: a worker closes on the
lane's green with no verify run of its own (air-80x.1). A bead with a commit after the cut is
refused, naming that commit. Landing stays the coordinator's: `air land --worker <lane>` lands
the batch and attributes every bead its range names by trailer (air-80x.2). The in-flight
refusal treats the lane's verify like any other (air-4cr). **[Air enforces]**

**Two ordering facts.** A batch is cut at specific shas, so a worker's commit after the cut is
not in it and waits for the next. A lane merges branches it did not write, so a branch that
conflicts is dropped from the batch and named, never resolved by the lane. **[fact]**

Removed when verify is cheap enough (scoped, or under a minute) that a round shows no batch of
more than one branch; the role is a worker again.

## Coordinator (the main checkout, holding no lane)

Two modes (owner, 2026-08-21). **Active:** every online worker has work: keep the ready list
full of claimable tasks, set priority, add `blocks` edges for shared files. **Decomposing an
epic is something you go and do, not a property a good queue happens to have** (owner,
2026-09-06): when an epic has no open child, decompose it, using the `decomposition` skill.
The reading may be delegated to a background agent; the filing and the deciding are yours.
Nothing refuses on this and nothing pushes it — `air status` names each such epic with its
closed count, and that is the whole mechanism.
Never set `assignee` on an open
bead: in bd 1.2.x it blocks every other worker's claim. Workers pull; there is no cap on work in
flight. **Idle:** feed no one. Naming a bead at a worker reserves nothing: `air claim` is the
reservation, and a bead named in a message and not claimed is still every worker's to take
(the adopter lost two that way; owner, 2026-09-05). **[fact]**
**So put the craft notes on the bead, not in the message**: `bd comment <id> --file <notes>`.
Whoever pulls it gets what you know, the note outlives the session that wrote it, and you are
not relying on a reservation that does not exist. Naming the bead at a worker as well is fine —
it reserves nothing, so expect any worker to take it and say nothing that implies otherwise.
That sentence above stated only the consequence, and between 2026-09-05 and 2026-09-07 it was
read, agreed with, and worked around five times: two beads lost at an adopter, two dispatched
twice here, and once the Stop hook itself told an idle worker to claim a bead already spoken
for — correct instruction, correct inputs, invisible reservation (air-u3l7). A rule that names
a hazard without naming the alternative reads as "be careful", and careful is what everyone
already was. **Removed when** a round passes with no bead named at a worker outside its own
bead; if a sixth instance happens instead, prose has failed twice and the answer is recording
the offer with an expiry.
Ask the owner only for a genuine edge case (a blocker only they can clear, an ambiguous
acceptance, a resource conflict), by filing a bead labelled `owner` with your recommendation in
its description. Those beads are the owner's queue (air-uef); `air claim` refuses them to
workers, and `air status` counts them on its `ready:` line.

**Your context is the channel the owner and every worker reach, so keep it free.** Long reads,
dry runs and analyses go to a background agent with a file deliverable; the filing and the
deciding stay yours. A coordinator inside a twenty-minute read is a fleet with no one to talk
to: on 2026-09-06 it was the only path to the owner and to four workers while it sat in a 21 GB
copy and a five-minute verify, and messages queued behind both. This is a fact about where the
coordinator's attention has to be, not a procedure (owner, 2026-09-06, air-zth). Removed when a
round shows zero owner or worker messages waiting more than five minutes on the coordinator.

**Where a bead came from is a declared field, not a memory.** Where the repo attaches a
planning tool to this session (`"metis": true` in `.claude/air.json` attaches Metis, and no
worker ever gets it), plan there and file beads from it: each bead's description carries a line
reading `initiative: <CODE>`. Air reads that line and nothing else — a mention of an initiative
in prose declares nothing. **[fact]** `air status` prints how many beads declare none, over the
set it already asked bd about. It is a count and there is no refusal attached to it; a gate
comes only if the count shows the rule is ignored (owner, 2026-09-06, air-g5o). Removed when
`bd create --validate` can require the field.

Your inputs are facts, not relayed memory: `air status` (sessions, claims, green at HEAD,
landable branches, ready depth with the owner-labelled count, every ready epic with no open
child and its closed count, beads without an initiative, leases, inbox depth),
`air holdings`, the channel (idle or silent with a claim, idle without a claim, hand-over
not green, landable branch, lease held by a dead session, session joined or left). **[fact]** A
condition pushes only when the SET changes, not while it ages; the facts themselves are always
in `air status` on demand (air-s7c, 2026-08-22). What each mechanism costs and the condition
under which it goes: `air audit`.

**Keep the same journal a worker does, and you are the reason it exists.** The round log is
assembled from your memory of messages, and a coordinator that hits a limit, compacts or ends
loses it — one did on 2026-09-06. Write what the next coordinator would want and the beads will
not carry: a claim of yours that turned out wrong, a ruling and what changed it, a thing you
nearly filed and why you did not. Same file shape, same `journal_dir`, and nothing reads it.
**[fact]**

**You need the same recurring wake a worker does, for the same reason and more urgently.**
On 2026-09-06 Air detected the stopped lane and pushed `silent-with-claim` to every session at
06:55:21; the coordinator was itself limited at that moment, and after its own reset it took the
problem to the owner instead of messaging the lane, which cost 53 further minutes. Its heartbeat
is what brought it back at all: five firings through the outage, and the sixth, four seconds
after the reset window opened, did real work. **[fact]**

**What Air records about a stopped session, and what it does not do about it.** A `Notification`
or `StopFailure` hook writes `stopped_at`, `stopped_kind` and `stopped_text` on the session row
(schema v20), and `air status` prints `STOPPED at <t>` on that session's line with the kind
spelled out. Nothing is refused, nothing is woken, nothing is relaunched: it is a fact for you to
act on. **Read the kind before acting, because the two states call for opposite moves**: a
`quota_auto_resume_fired` session is being brought back by the harness and typing at it CANCELS
that recovery, while `quota_auto_resume_stale`, `quota_auto_resume_disabled` and `stop_failure`
mean nothing is coming and one message is right. Silence alone is not the trigger; the kind is
(air-1n3). **[fact]**

Workers are reached with `SendMessage` to the session name `air status` shows; tmux panes are
for the owner to watch, not for the coordinator to type into (send-keys was allowed once and
denied 30 min later by the permission classifier, 2026-08-22; removed when a round passes with
zero denied send-keys attempts). **[fact]**
**A 5-minute heartbeat runs for the whole round.** The channel pushes conditions when the set
changes; the heartbeat is the failsafe. It runs `air status` and says nothing when nothing
changed. A wedged worker reaches nobody by itself: `stuck`, the condition that promised to
catch one, was set only by a permission prompt the fleet's auto mode never shows, fired zero
times in any recorded day, and was deleted on 2026-09-06 (air-12k) after the heartbeat did
every catch in the 2026-09-05 round. Incident: the 2026-08-22 05:26-05:45 standstill (air-arq),
where the quiet channel rested on the coordinator remembering to look. Removed when a condition
catches a real wedge before the heartbeat does, twice. **[fact]**

**Landing is the coordinator's, not a worker's.** *How* a branch reaches main is the repo's own
flow and lives in its CLAUDE.md, exactly as hand-over does: some repos have their own lander,
some use Air's. Air names no landing command here (air-97z).

What Air states either way: a branch is landable when it carries a **recorded green at a head
that contains `main`**; the beads a landing carries are the ones its commits name in a
`Bead: <id>` trailer, so put the trailer on every commit that does a bead's work — no bead status
is consulted, prose is not read, and a commit without one is attributed to nothing (air-7kp,
air-4re); and Air records every landing it performs. `air status` names what is ready.
**[Air enforces, when the landing is Air's: main checkout, on main, branch contains main,
recorded green at the branch head, and no verify in flight anywhere in the fleet — a landing
moves `main` and destroys every run in progress; `air land --despite-inflight` lands anyway and
the runs it destroyed are recorded on the landing (air-1bm)]**
**Air's landing does not re-verify, and main never holds a commit that has not been verified**
(air-odv, 2026-08-29). The landing commit is built off main with `git commit-tree` and main is
fast-forwarded onto it. Because the branch must contain main, that commit's tree is
byte-identical to the one the worker recorded its green for, so there is nothing new to verify —
and nothing to roll back, no armed window, and no `git reset --hard` on main. The clean-tree
refusal went with the reset that was its only reason. **[fact]** An identical tree is an
identical verdict only when the repo's verify reads the tree alone and not git history; a
verify that reads the log, the branch name or the reflog can pass on the branch and fail on
main over the same bytes (the adopter; owner, 2026-09-05). **[fact]**

**What main moving costs, and what it no longer costs** (air-9ij, 2026-09-06). Landability is
the thing that expires: a branch is landable only while it contains CURRENT main, so every
write to main takes that away from every other branch at once, and each of those workers pays a
merge and a re-verify to get it back. Two consequences, and they are facts about the refusal,
not advice about tempo. **Landing order:** landing several branches in a row costs the second
one its landability the moment the first lands; a batch lands once and costs it once.
**The coordinator's own commits move main exactly as a landing does** — an adopter's
coordinator invalidated four workers' landability with one prose commit on 2026-09-06, with no
landing involved — so they cost whatever a landing costs, in the same units.
What has STOPPED being true: main moving no longer retracts a CLOSE. A green is checked against
the main it was recorded over, not against the main of the moment somebody asks, and a bead
whose commits are already in main closes on the landing that put them there. The workaround an
adopter ran that night — freezing main from the batch cut until every close was confirmed — was
buying exactly this and can go. **[Air enforces: the close asks about the recorded main, the
landing about current main]**

The `.git` shape still differs between a worktree (a FILE) and the main checkout (a DIRECTORY),
and anything reading it, `core.hooksPath`, or the cwd can differ between the two. That was a
reason to verify twice while a landing verified in the main checkout (air-eaw, from the adopter
2026-08-23: *"Every other instrument failure that night was catchable by running the suite. This
one was only catchable by running it somewhere else."*). Air's landing no longer runs anything
there, so the difference is now a reason to fix a test that reads where it runs, not a reason to
pay a second full verify per landing.
Merging is not closing, and a landing closes nothing: the worker closes its own bead with
proof (owner, 2026-08-22). A landing Air performs prints every bead beside its acceptance criteria and
Air's verdict on each clause, which is the only external check on that. Air discharges a clause
only by lookup (a recorded green at the landed sha, a path the merge changed) and reports the
rest as unreadable rather than judging prose. A clause the merge CONTRADICTS is a wrong close,
named by `air status` (air-ayp; the adopter closed 99 beads on containment alone, 14 partial and
1 not done). **[Air enforces]**

Intake: `air inbox` → `bd create --validate --estimate <min>` → `air triage <id> --bead <new>` or
`--drop "<why>"`. `--validate` refuses without these sections, per type: task/feature `##
Acceptance Criteria`; bug `## Steps to Reproduce` + `## Acceptance Criteria`; epic `## Success
Criteria` (`## Acceptance Criteria` accepted); chore none (bd `internal/types/types.go`
`RequiredSections`, main, read 2026-08-22; air-8zz). **[fact]** Workers request beads this way,
including friction beads; they never create them.
Every capture is triaged into a bead or dropped with a reason; nothing a worker writes reaches
the owner unfiltered (air-uef, owner 2026-09-05: two queues reached the owner, and the prose one
carried no id, no acceptance and no recommendation). A bead the owner must decide is
labelled `owner` with the coordinator's recommendation in its description; `air claim` refuses
it to workers (the gate is `owner`, not `human`: `human` is presence, `owner` is authority;
owner, 2026-08-22). Launch workers
yourself with `air worker <name> --tmux --task "<complete task>"` (an attachable pane the owner
can open).

Not available to the coordinator, by deny rule: `git push`. **The boundary is the remote, not
main.** The coordinator may commit and merge on main — its own prose is its own to save, and
whatever path this repo lands by, Air pushes nothing: a landing it performs reaches main and
stops there. Landing a *worker's* branch is still that path, not a hand merge.
**[Air enforces: `git push`]**

## When refused

A refusal names its rule and the fixing command. Silence from Air is not a denial.

## Provenance

Cut to facts and refusals after `../research/guardrails-as-throttles.md` (2026-08-21): advice
to a capable model was removed; what remains is what Air records, answers, or refuses. Duties
adapted from the adopter's `main-agent-protocol.md` and `worktree-protocol.md`; decisions in
`../decisions.md` (2026-08-20/21). Standstill lines (worker run-to-completion, from
The adopter's CLAUDE.md:683; coordinator reach, poll, and landings) added for the 2026-08-22
05:26-05:45 incident, bead air-arq. This file is embedded in the `air` binary (`ROLES_MD`,
`include_str!`) and written to `.air/roles.md` by `air init`; the two cannot differ.
