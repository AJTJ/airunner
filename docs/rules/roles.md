# Roles: coordinator and worker

> Read at session start. Markers: **[Air enforces]** a refusal, deny rule, or native block;
> **[fact]** something Air records or answers; everything else is yours to judge. Domain rules
> AND the repo's own work flow — how a finished bead is handed on — live in the repo's
> CLAUDE.md, not here; Air states what it records and what it refuses. Background:
> [`../research/agent-roles-and-confinement.md`](../research/agent-roles-and-confinement.md).

## Which role am I

Role is the checkout: `[ -f .git ] && echo worker || echo coordinator`. The main checkout is the
coordinator; each worktree is a worker. Air records it on every session and event. **[fact]**
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
"next bead" as "wait for review" and idled 20 min, 2026-08-22; adopter's 51-minute idle of
2026-08-15 was fixed by this sentence and never recurred. Removed never.) The run-on begins at
your first piece of work, not at session start; the two sentences above are the whole of the
difference.

Claiming a bead is a commitment to work it to completion now: `air claim <id> [--files a,b]`,
then the work. **How a finished bead is handed on is the repo's own flow, in its CLAUDE.md,
not Air's to prescribe** — some repos hand over for review, some close with proof.

Two constraints, because Air checks them and they are about ordering rather than procedure:

- **The recorded green has to be at the commit you hand on, and that commit has to contain
  `main`.** So merge `main` first and run `air record verify -- <cmd>` last; a green recorded
  before the merge is a green for a tree nobody will land.
- **A digest, where the repo configures `digest_dir`, has to name its bead** in front matter
  (`---` / `bead: <id>` / `---`) and be written with the work rather than after the fact. Air
  reads the declared field, not the filename: a digest for another bead used to satisfy the
  gate (air-agq).

`air handover` names whatever is missing and the command that fixes it, so run it before you
finish rather than guessing which of the two bit you. **[fact]**

A closed bead stays closed; unfinished work is a new bead that references it (ask via
`air capture`). Stop only for a genuine blocker or an owner-only decision; say so in one line
with `air capture "<blocker>"` (or `--for owner`), then `air release <id> --reason <why>` or
take unrelated work.

Facts available to you: `air holdings` (who is in which file), `air status`, `air lease status`,
`air handover` (what is missing and the command that fixes it). A warning that a peer holds a
file you are opening arrives once per session. **[fact]**

Things that need a shared resource (a port, the simulator, Docker, the browser):
`air lease take <resource> --reason "<why>"`; release when done. A held lease names its holder;
do not route around it. **[Air enforces: a healthy holder is not broken by `take`]**

Not available to a worker, by deny rule in every permission mode: `air land`, `air close`,
`git push`, `bd create`, `bd sync`, raw `bd update --claim`, a nested `claude`, leaving the
worktree. Editing
the main checkout is blocked natively. **[Air enforces]** The one refusal: the `bd` write that
ends your work on a bead — `bd close`, or `bd update -s closed` / `-s awaiting_review`,
whichever your repo uses — is denied without a recorded green at HEAD that contains `main`.
Worker launches set `AIR_ENFORCE=1` and the hook names the fixing command. Enforced after the
first bypass of the advisory gate (tty-fix, 2026-08-22 06:00, air-i59); removed when a full
round passes with zero `handover-not-green` events. **[Air enforces]**

## Coordinator (the main checkout, holding no lane)

Two modes (owner, 2026-08-21). **Active:** every online worker has work: keep the ready list
full of claimable tasks (epics decomposed; the reading may be delegated, the filing and deciding
are yours), set priority, add `blocks` edges for shared files. Never set `assignee` on an open
bead: in bd 1.2.x it blocks every other worker's claim. Workers pull; there is no cap on work in
flight. **Idle:** feed no one.
Ask the owner only for a genuine edge case (a blocker only they can clear, an ambiguous
acceptance, a resource conflict), through the owner queue.

Your inputs are facts, not relayed memory: `air status` (sessions, claims, green at HEAD, review
waits, ready depth, leases, inbox depth), `air holdings`, the channel (stuck, idle or silent
with a claim, idle without a claim, hand-over not green, review waiting, lease held by a dead
session, owner decision waiting, session joined or left). **[fact]** Review waiting and owner
decision waiting push only when the SET changes, not while it ages; the waits themselves are
always in `air status` and `air inbox --owner` on demand (air-s7c, 2026-08-22). What each
mechanism costs and the condition under which it goes: `air audit`.

Workers are reached with `SendMessage` to the session name `air status` shows; tmux panes are
for the owner to watch, not for the coordinator to type into (send-keys was allowed once and
denied 30 min later by the permission classifier, 2026-08-22; removed when a round passes with
zero denied send-keys attempts). **[fact]**
**A 5-minute heartbeat runs for the whole round.** The channel pushes conditions on change, and
`stuck` — the one that should catch a wedged worker — has never fired in any recorded day and
carries no removal condition (air-dqw). So a wedged worker can reach nobody. The heartbeat is the
failsafe, not the reporting path: it runs `air status` and says nothing when nothing changed.
Incident: the 2026-08-22 05:26-05:45 standstill (air-arq), where the quiet channel rested on the
coordinator remembering to look. Removed when `stuck` fires on a real wedge before the heartbeat
catches it, twice. **[fact]**

Landing is the coordinator's: `air land --all` merges every green branch into main,
oldest first, verifies the merged result, and puts main back where it was on red (air-3pz). A
branch is landable when it carries a recorded green at its head; the beads reported are the
ones its commits name in a `Bead: <id>` trailer — no bead status is consulted, and prose is not
read (air-7kp, air-4re). Put the trailer on every commit that does a bead's work; a commit
without one is attributed to nothing. `air status` names what is ready. **[Air enforces: main checkout, on main, clean
tracked tree, branch contains main, recorded green at the branch head]**
**The landing verify is not a repeat of the worker's: it runs in the main checkout, where the
environment differs.** `.git` is a FILE in a worktree and a DIRECTORY in the main checkout, and
anything reading its shape, `core.hooksPath`, or the cwd can differ between the two. adopter
2026-08-23: a probe was green in the worktree and red on the merged result for exactly that
reason, and in the main checkout it did not refuse, so the target RAN and wrote hooks during the
landing verify. "Every other instrument failure that night was catchable by running the suite.
This one was only catchable by running it somewhere else." A worker has exactly one environment,
so a green branch going red on landing is information, not a flake. **[fact]**
Merging is not closing, and `air land` closes nothing: the worker closes its own bead with
proof (owner, 2026-08-22). The landing prints every bead beside its acceptance criteria and
Air's verdict on each clause, which is the only external check on that. Air discharges a clause
only by lookup (a recorded green at the landed sha, a path the merge changed) and reports the
rest as unreadable rather than judging prose. A clause the merge CONTRADICTS is a wrong close,
named by `air status` (air-ayp; adopter closed 99 beads on containment alone, 14 partial and
1 not done). **[Air enforces]**

Intake: `air inbox` → `bd create --validate --estimate <min>` → `air triage <id> --bead <new>` or
`--drop "<why>"`. `--validate` refuses without these sections, per type: task/feature `##
Acceptance Criteria`; bug `## Steps to Reproduce` + `## Acceptance Criteria`; epic `## Success
Criteria` (`## Acceptance Criteria` accepted); chore none (bd `internal/types/types.go`
`RequiredSections`, main, read 2026-08-22; air-8zz). **[fact]** Workers request beads this way,
including friction beads; they never create them. Owner queue: `air inbox --owner`; a bead
labelled `owner` is awaiting the owner and `air claim` refuses it to workers (the gate is
`owner`, not `human`: `human` is presence, `owner` is authority; owner, 2026-08-22). Launch workers
yourself with `air worker <name> --tmux --task "<complete task>"` (an attachable pane the owner
can open).

Not available to the coordinator, by deny rule: `git commit` and `git push` on main. `air land`
is the one allowed path onto main, and it pushes nothing.
**[Air enforces]**

## When refused

A refusal names its rule and the fixing command. Silence from Air is not a denial.

## Provenance

Cut to facts and refusals after `../research/guardrails-as-throttles.md` (2026-08-21): advice
to a capable model was removed; what remains is what Air records, answers, or refuses. Duties
adapted from adopter's `main-agent-protocol.md` and `worktree-protocol.md`; decisions in
`../decisions.md` (2026-08-20/21). Standstill lines (worker run-to-completion, from
adopter/CLAUDE.md:683; coordinator reach, poll, and landings) added for the 2026-08-22
05:26-05:45 incident, bead air-arq. This file is embedded in the `air` binary (`ROLES_MD`,
`include_str!`) and written to `.air/roles.md` by `air init`; the two cannot differ.
