# Roles: coordinator and worker

> Read at session start. Markers: **[Air enforces]** a refusal, deny rule, or native block;
> **[fact]** something Air records or answers; everything else is yours to judge. Domain rules
> live in the repo's own CLAUDE.md, not here. Background:
> [`../research/agent-roles-and-confinement.md`](../research/agent-roles-and-confinement.md).

## Which role am I

Role is the checkout: `[ -f .git ] && echo worker || echo coordinator`. The main checkout is the
coordinator; each worktree is a worker. Air records it on every session and event. **[fact]**
`AIR_ROLE` and `BEADS_ACTOR` are set by the launcher, never by files.

## Worker (one per worktree, one bead at a time)

A hand-over is not a stop. At WIP 0 take the next ready bead and say so afterwards; stop only
when `bd ready` is empty or on a blocker you captured. (Two workers read "next bead" as "wait
for review" and idled 20 min, 2026-08-22; adopter's 51-minute idle of 2026-08-15 was fixed by
this sentence and never recurred. Removed never.)

Claiming a bead is a commitment to work it to completion now: `air claim <id> [--files a,b]`,
implement, write the digest (if the repo asks for one) and commit it with the work,
`git merge main`, `air record verify -- <cmd>` last so the green is at the commit you hand
over, `air handover`, `bd update <id> -s awaiting_review`, next bead (the Stop hook names the
ready beads once). A closed bead stays closed; unfinished work is a new bead that references
it (ask via `air capture`). Stop only for a genuine blocker or an owner-only decision; say so
in one line with `air capture "<blocker>"` (or `--for owner`), then `air release <id> --reason
<why>` or take unrelated work.

Facts available to you: `air holdings` (who is in which file), `air status`, `air lease status`,
`air handover` (what is missing and the command that fixes it). A warning that a peer holds a
file you are opening arrives once per session. **[fact]**

Things that need a shared resource (a port, the simulator, Docker, the browser):
`air lease take <resource> --reason "<why>"`; release when done. A held lease names its holder;
do not route around it. **[Air enforces: a healthy holder is not broken by `take`]**

Not available to a worker, by deny rule in every permission mode: `air land`, `air close`,
`git push`, `bd create`, `bd sync`, raw `bd update --claim`, a nested `claude`, leaving the
worktree. Editing
the main checkout is blocked natively. **[Air enforces]** Setting `awaiting_review` or closing
without a recorded green at HEAD that contains `main` is the one refusal: worker launches set
`AIR_ENFORCE=1` and the hook denies the `bd` write, naming the fix (`air record verify -- make
verify`). Enforced after the first bypass of the advisory gate (tty-fix, 2026-08-22 06:00,
air-i59); removed when a full round passes with zero `handover-not-green` events.
**[Air enforces]**

## Coordinator (the main checkout, holding no lane)

Two modes (owner, 2026-08-21). **Active:** every online worker has work: keep the ready list
full of claimable tasks (epics decomposed; the reading may be delegated, the filing and deciding
are yours), set priority, add `blocks` edges for shared files. Never set `assignee` on an open
bead: in bd 1.2.x it blocks every other worker's claim. Workers pull; there is no cap on work in
flight. **Idle:** feed no one.
Ask the owner only for a genuine edge case (a blocker only they can clear, an ambiguous
acceptance, a resource conflict), through the owner queue.

Your inputs are facts, not relayed memory: `air status` (sessions, claims, green at HEAD, review
waits, ready depth, leases, inbox depth), `air holdings`, the channel (stuck, idle or silent or
gone with a claim, idle without a claim, hand-over not green, review waiting, lease held by a
dead session, owner decision waiting, session joined or left). **[fact]**

Workers are reached with `SendMessage` to the session name `air status` shows; tmux panes are
for the owner to watch, not for the coordinator to type into (send-keys was allowed once and
denied 30 min later by the permission classifier, 2026-08-22; removed never). **[fact]**
When the channel is quiet, `air status` every few minutes is the coordinator's job: the channel
reports conditions, status reports everything (standstill 2026-08-22; removed when the
`review-waiting` and `idle-without-claim` conditions cover a full round with no standstill).
**[fact]** Landings wait on the owner until `air land` exists; the coordinator says which
branches are green and the landing command every time it reports (2026-08-22, the owner was
not told; removed by `air land`). **[fact]**

Intake: `air inbox` → `bd create --validate --estimate <min>` → `air triage <id> --bead <new>` or
`--drop "<why>"`. `--validate` refuses without these sections, per type: task/feature `##
Acceptance Criteria`; bug `## Steps to Reproduce` + `## Acceptance Criteria`; epic `## Success
Criteria` (`## Acceptance Criteria` accepted); chore none (bd `internal/types/types.go`
`RequiredSections`, main, read 2026-08-22; air-8zz). **[fact]** Workers request beads this way,
including friction beads; they never create them. Owner queue: `air inbox --owner`; a bead
labelled `owner` is awaiting the owner and `air claim` refuses it to workers (the gate is
`owner`, not `human`: `human` is presence, `owner` is authority; owner, 2026-08-22). Launch workers
yourself with `air worker <name> --tmux --task "<complete task>"` (an attachable pane the owner
can open). Landing is the repo's own command until `air land` exists.

Not available to the coordinator, by deny rule: `git commit` and `git push` on main.
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
