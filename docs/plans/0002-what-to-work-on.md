# 0002 — What to work on: feature → epics → beads, and traversing an epic

Status: **draft**, 2026-08-18. Research behind it:
`docs/research/verification/ticks/2026-08-18-0230-what-to-work-on.md` (comparison table, sources).
Companion: `docs/plans/0001-first-slice.md` (the ledger, hooks, hand-over gate); skills
`decomposition`, `phase-transitions`, `beads` in `.claude/skills/`.

## 1. Scope

- **The owner picks the feature.** Not automated, not tracked by Air (`docs/decisions.md:42-45`;
  `.claude/skills/phase-transitions/SKILL.md:19`). A feature is a paragraph in `docs/decisions.md`
  or a plan file; nothing more.
- This plan covers what happens after: cutting the feature into epics and beads with a real
  commitment point, and letting a few workers plus a coordinator walk an epic to done, with the
  least machinery that the record supports.
- Principle carried over: **enforce evidence-shaped checks; keep judgement as skill.** adopter's
  lesson is that positive, sequential rules stay prose unless a check owns them (SYNTHESIS §1),
  and its warning is that a phase machine "will be exactly that kind of wrong the first time real
  work does not fit its model" (`adopter-notes/plans/0022-agent-working-procedure.md:236-240`).

## 2. Decomposition and the triage commitment point

### 2.1 Feature → epics (skill: `decomposition`)

- Decompose **ahead of capacity**: cut the next epic when `air next` / `bd ready` is thinning, not
  the whole feature at once (Metis, `decomposition/SKILL.md:38-49`; spec-kit "MVP increment").
- One epic = one capability increment, vertical slice preferred; risk-first spike when the
  approach is unknown (`decomposition/SKILL.md:98-160`).
- The epic's `--design` is its spec, self-contained (Anthropic best practices: "name the files and
  interfaces involved, state what is out of scope, and end with an end-to-end verification step"):
  approach, files/dirs it will own (the lanes), out of scope, risks, and **one end-to-end check
  (a command)** that says the epic is done. Written with `--design-file -`
  (`beads/SKILL.md:201-204`). Non-empty `--design` is required before any child can be claimed
  (SYNTHESIS §4.3 check 8).
- If the whole thing fits one sentence of diff, it is a bead, not an epic (Anthropic: "If you could
  describe the diff in one sentence, skip the plan").

### 2.2 Epic → beads: the commitment point (enforced)

Today `bd create` equals `bd ready` (0022 §2). Air adds the line Kanban and Symphony have (Backlog
→ Todo is a human move; "Backlog -> out of scope for this workflow; do not modify",
`openai/symphony elixir/WORKFLOW.md:109`). A child bead is *triaged* — offered by `air next`,
claimable — only when all four hold; until then it is a capture, an option:

| # | Requirement | Why | Mechanism |
|---|---|---|---|
| 1 | `--acceptance` = one observable condition runnable inside the worktree by the agent — the check (command / test name), not prose | task-spec §3; verification: "acceptance criteria are the weak link … executable, red/green-probed" | `bd create --validate` (refuses empty); `air next` filters on presence; probe later |
| 2 | Lane label = the files/dirs it will touch; `owner`/`human`/`runtime` labels applied at filing | corpus §0.4 partition by file; `beads/SKILL.md` "label owner at filing" | `air next` filters; `air claim` records the lane (check 6) |
| 3 | Edges: `--parent <epic>`; `blocks` for ordering and for any file shared with an open sibling; `bd dep cycles` empty | 0022 §3 (three missing edges "nobody had seen"); beads: only edges sequence children | `air next` requires parent; `bd dep cycles` in `air doctor` |
| 4 | Citations (`file:line`) resolve at filing; description and acceptance agree (else `bd human`) | task-spec §3 stop rule; ConInstruct: contradictions are silent | `air next` citation check (check 5) |

Who does it: the coordinator, or a dispatched triage session holding no lane, on a cadence
(0022 §4 "Who runs triage"). Discovered work mid-task is a one-line capture with `discovered-from`,
never a full bead on the hot path. If captures outrun triage, auto-file with `triage-needed`
rather than abandon the line (0022 §8).

## 3. Epic traversal (few workers + coordinator)

- **Ready frontier** = `bd ready --parent <epic>` (children parallel by default; only edges
  sequence — beads `molecules.md:79-99`). Order = priority, then oldest, then id (Symphony SPEC
  §8.2; `bd ready --sort`). "Foundational first, then slices in parallel" (spec-kit Phase 2, Gas
  Town waves) is expressed with `blocks` edges, never with phases.
- **Concurrency**: a few workers (≤4, corpus §0.6); WIP ≤2 per worker (check 9); one live claim per
  lane (check 6). Symphony's per-state caps reduce to one candidate number for Air — how many
  beads may wait in `awaiting_review` — which the ledger measures first (review latency, plan 0001
  §8) before anyone caps it.
- **Claim** (`air claim`): CAS via `bd`, lane declared, citations re-verified; a stale citation
  closes the bead with the finding (`beads/SKILL.md:243-246`).
- **Re-verify citations**: at claim, and after any merge touching cited files (`air post-merge`,
  plan 0001 §3). When a sibling lands, `air next` re-checks the open siblings' citations and prints
  "stale since <sha>".
- **When a result re-cuts siblings** (coordinator judgement; Air only flags): a spike closes →
  decompose the rest now; a landed sibling changed cited files or already satisfies a sibling's
  acceptance → close it with the evidence ("if it is done, close it; finding that out is the
  work", `beads/SKILL.md:247-251`) or re-file with `supersedes`; description/acceptance
  contradiction → `bd human`.
- **No stalls**: no hook blocks on a question; a claimed bead that cannot progress is released
  (Symphony §10.5 "MUST NOT stall indefinitely"; corpus "deadlines ask, don't kill").
- Landing stays piecemeal to `main` via `land` (adopter's proven gate); no per-epic integration
  branch until a measured "A landed and broke B" incident.

## 4. Gates and done

- **The one gate** is the hand-over refusal from plan 0001 §4 (recorded green at HEAD + main
  merged); it is the same for children of an epic as for any bead.
- **Human decisions**: `bd human <id>` / `owner` label (withheld from `next`). No gate beads
  (`human/timer/gh:*`) until a named pain; if one appears, bd's `human` gate is the drop-in.
- **Child done**: `bd close --reason` with landed sha + acceptance evidence, only after `land`
  (`phase-transitions/SKILL.md`).
- **Epic done**: every child closed or superseded **and** the epic's end-to-end check (from
  `--design`) recorded green on `main`; then `bd close <epic> --reason` naming the shas.
  `bd epic close-eligible` is the sweep; the check is Air's extra condition. Progress:
  `bd epic status` + `air status`.

## 5. What Air enforces vs what stays coordinator skill

| Air enforces (evidence, cheap, probed) | Coordinator / skill (judgement) |
|---|---|
| Non-empty `--design` before a child claim (check 8) | Which feature; which epic next; priorities |
| Triage commitment: `air next` offers only beads with acceptance + lane/labels + parent/edges + resolving citations (check 4) | Writing the acceptance check; choosing lanes; deciding drop vs file |
| Citation check at claim and post-merge (check 5); lane at claim (check 6); WIP (check 9) | Vertical vs horizontal cut; spike first; sizing |
| Hand-over refusal (plan 0001 §4) | Re-cutting siblings; superseding; `bd human` rulings |
| Epic close requires all children closed/superseded + epic check green on main | When to open the next epic; wave ordering via edges |
| Ledger metrics: time design→first claim, time in `awaiting_review`, stale-citation flags, capture depth / time-to-triage | Reading them |

## 6. What NOT to build

Formulas/protos/molecules/wisps engine; convoys; per-epic integration branches / refinery; a
code-enforced epic phase machine; a spec/plan/tasks markdown triple; an LLM decompose/triage
daemon; automated feature selection; timer/gh gates and per-state caps for every state; task
checklists and `waits-for` fan-in beads; automatic re-cutting. Reasons and citations: tick note §4.

## 7. Open decisions for the owner — ANSWERED 2026-08-18 (see `docs/decisions.md`)

Answers: (1) before children; (2) inline; (3) `--validate` on, and *workers capture, they do not file* — triage is a separate pass that validates/dedupes/groups before promotion; (4) tabled — derive, don't store; (5) measure, don't enforce; (6) not pursued. Original questions kept below for the record.


1. Is the epic's own end-to-end check (a command in `--design`) required before children may be
   claimed, or only before the epic may close? (Recommended: required at design; it is the spec.)
2. Who triages: the coordinator inline, or a dispatched triage session on a cadence (0022)? Start
   inline; dispatch when capture depth shows up in the ledger?
3. Adopt `bd create --validate` in adopter now (refuses missing acceptance) — cheap, reversible.
4. Epic phases: not tracked at all, or a `phase:<name>` label maintained by the skill for
   visibility? (Recommended: not tracked; `--design` present + children triaged are the two facts.)
5. Whether to cap `awaiting_review` count after one round of latency data.
6. Trigger for revisiting integration branches: first measured cross-sibling breakage on `main`.

## 8. Sources

Tick note `docs/research/verification/ticks/2026-08-18-0230-what-to-work-on.md` §5 (full list).
Primary external, accessed 2026-08-18: openai/symphony `SPEC.md` and `elixir/WORKFLOW.md`;
gastownhall/gastown `docs/concepts/{convoy,integration-branches,molecules}.md`; steveyegge/beads
`docs/workflows/{molecules,gates,formulas}.md`; github/spec-kit `README.md` and
`templates/tasks-template.md`; https://code.claude.com/docs/en/best-practices;
https://learn.chatgpt.com/guides/best-practices. Local: Metis ADR-003/ADR-007
(`~/projects/metis/.metis/adrs/`), the ported skills, `docs/research/metis-deep-dive.md`,
`docs/research/beads-and-gastown.md`, adopter 0022 and task-specification notes,
`docs/research/verification/specs-guards-tooling.md`, `docs/research/SYNTHESIS.md`.
