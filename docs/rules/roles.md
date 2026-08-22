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

Claiming a bead is a commitment to work it to completion now: `air claim <id> [--files a,b]`,
implement, write the digest (if the repo asks for one) and commit it with the work,
`git merge main`, `air record verify -- <cmd>` last so the green is at the commit you hand
over, `air handover`, `bd update <id> -s awaiting_review`, next bead. A closed bead stays
closed; unfinished work is a new bead that references it (ask via `air capture`). Stop only for a genuine blocker or an owner-only
decision; say so in one line with `air capture "<blocker>"` (or `--for owner`), then `air release
<id> --reason <why>` or take unrelated work.

Facts available to you: `air holdings` (who is in which file), `air status`, `air lease status`,
`air handover` (what is missing and the command that fixes it). A warning that a peer holds a
file you are opening arrives once per session. **[fact]**

Things that need a shared resource (a port, the simulator, Docker, the browser):
`air lease take <resource> --reason "<why>"`; release when done. A held lease names its holder;
do not route around it. **[Air enforces: a healthy holder is not broken by `take`]**

Not available to a worker, by deny rule in every permission mode: `air land`, `git push`,
`bd create`, `bd sync`, raw `bd update --claim`, a nested `claude`, leaving the worktree. Editing
the main checkout is blocked natively. **[Air enforces]** Setting `awaiting_review` or closing
without a recorded green at HEAD that contains `main` is the one refusal (advisory this round).
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
waits, leases, inbox depth), `air holdings`, the channel (stuck, idle or silent or gone with a
claim, hand-over not green, lease held by a dead session, owner decision waiting, session
joined or left). **[fact]**

Intake: `air inbox` → `bd create --validate --estimate <min>` (bd refuses without `## Acceptance
Criteria`) → `air triage <id> --bead <new>` or `--drop "<why>"`. Workers request beads this way,
including friction beads; they never create them. Owner queue: `air inbox --owner`; a bead
labelled `human` is awaiting the owner and `air claim` refuses it to workers. Launch workers
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
`../decisions.md` (2026-08-20/21).
