---
name: decomposition
description: Use when asked to "break down this feature", "decompose this epic", "create beads from an epic", "how to size beads", "when to decompose", "vertical slices", "task granularity", "cut the next wave", "build the worker queues", or when an epic is about to be opened for claiming and needs children. Guides breaking a feature into epics and epics into claimable beads, in Air's vocabulary, using Metis's decomposition reasoning plus the agile splitting rules behind it (INVEST, story splitting, walking skeleton).
metadata:
  version: 1.1.0
---

# Work Decomposition

Breaking a feature into epics and an epic into claimable beads, then cutting per-worker queues
from the result. The reasoning is Metis's (`decomposition` skill, Flight Levels as Kanban) plus
the agile sources it draws on; the vocabulary, the checks, and the `bd` commands are Air's.
Where the text says "epic" or "bead", Metis says "initiative" or "task". Research and sources:
`docs/research/metis-decomposition-and-agile.md`.

## Vocabulary map (Metis → Air)

| Metis | Air | Notes (Air's, not Metis's) |
|---|---|---|
| Vision | **feature** | Owner picks it; not tracked (`docs/decisions.md` 2026-08-17). A paragraph in `decisions.md` or a plan. |
| Initiative | **epic** | `bd create --type=epic` with `--design` as its spec. Non-empty `--design` before any child can be claimed (SYNTHESIS §4.3 check 8; owner 2026-08-18 item 1). |
| Task | **bead** | One agent, one session, one reviewable diff, one runnable acceptance (`beads` skill). |
| Backlog item (bug/feature/tech-debt) | **capture** | One line, not `ready`, no acceptance. Workers capture; they never file (`decisions.md` 2026-08-18 item 3). |
| Initiative `discovery → design → ready → decompose → active → completed` | epic `discovery → design → decompose → triaged → active → closed`, **derived, never stored** | Metis's `ready` is Air's triage commitment point on the children. See `phase-transitions`. |
| "decompose phase is a visible buffer" | same | Ledger metric: time from `--design` to first child claim. |
| `estimated_complexity` XS-XL | `--estimate <minutes>` on beads, recorded not gated | adopter: "the missing instrument is a size estimate at filing time" (`overnight-fleet-retrospective.md:726`). |

## When to decompose (Metis)

Ahead of capacity, never upfront: when `bd ready --parent <epic> -n 0` has fewer open beads
than idle workers, or when the current wave is landing. Do not decompose everything (waterfall);
do not decompose before `--design` is written (premature decomposition: beads that solve the
wrong problem).

## Sizing (Metis's "scope, not time", with Air's unit)

**The unit is one worker session that ends with `air handover` succeeding.** Not hours, not
points. Scrum's one sizing rule is that the doers size; here the doer is a fresh session that
cannot renegotiate scope, so the filer sizes for it and the ledger's single-agent success rate is
the feedback.

| Level | Test |
|---|---|
| **Bead** | One deliverable; acceptance without "and"; one lane; no choice between approaches left to the worker; diff reviewable in one sitting. |
| **Epic** | One capability increment with ONE runnable "Done when" command. Two commands = two epics. |
| **Feature** | Several epics. Owner's. |

Filing-time signals:

- Acceptance needs "and" → split (`beads` skill).
- Checklist in the description → an epic that was not cut (`bd ready` cannot see or claim items;
  adopter `task-specification-research.md:571-581`, "epics wearing task clothes").
- Touches two lanes → split by lane, or record the edge and accept serial landing.
- Worker would have to pick an approach → spike first, or decide it in the description.
- Many unanswerable questions while writing the acceptance → too uncertain; spike or capture.
  Many rules → too big. One rule with many examples → a hidden second rule
  (`task-specification-research.md:265`).
- **Floor as well as ceiling**: not below one reviewable diff; "a bead too small to review as one
  change costs a full merge cycle for a trivial diff" (`task-specification-research.md:757-759`).
- Prefer three small beads to one medium; they parallelise. More than ~12 children → probably
  two epics.
- Set `--estimate <minutes>`; it is a guess the ledger correlates with actuals, never a gate.

## The procedure (coordinator runs this)

Inputs: the feature paragraph; `main`; `bd list --type=epic`; `bd ready -n 0`; the capture
inbox; `air status` (lanes held, idle workers). Output: one epic, the skeleton plus one wave of
triaged children with edges, and per-worker assignments. Never the whole feature.

### 1. Frame the epic (Metis discovery + design, collapsed into `--design`)

`design.md` with exactly these headings; each maps to a Metis exit criterion or an Air check:

```
# <epic title>
## Why            named pain from the record, with citation          [Metis discovery exit]
## Approach       how; alternatives rejected, one line each           [Metis design exit]
## Lanes          files/dirs this epic owns, one line per lane        [Air check 6]
## Out of scope   what a worker must capture, not do                  [feature creep rule]
## Risks          unknowns; each becomes a spike or a blocks edge     [risk-first]
## Done when      ONE command that exits 0 on main when complete      [epic close check]
## Later          slices deliberately not cut yet                     [one-wave rule]
```

"Done when" is required before any child may be claimed and may be tightened later, never
removed (Metis ADR-003:55 "criteria cannot be removed once defined, only refined"). If the epic
fits one sentence of diff, stop: file one bead under an existing epic.

```bash
bd create "<epic title>" --type=epic -p 2 --description "<why, one paragraph>" \
  --design-file - < design.md
```

### 2. Find the walking skeleton (Patton, Cockburn)

Child 1 is the thinnest end-to-end path that makes "Done when" *runnable*, even if it returns
the wrong answer: it "links together the main architectural components" and surfaces
integration risk first. If the approach is unknown, child 1 is `--type=spike` whose acceptance
is a written finding (`bd comment` naming the chosen approach and the rejected ones), and the
rest waits on it with `blocks` edges.

### 3. Split the rest: run down this list, stop at the first pattern giving 2-5 equal children

From Lawrence's story-splitting patterns; Metis has the vertical-over-horizontal preference but
not the list. Pick the split that lets you throw a child away, then the one with equal sizes.

1. **Workflow steps**: simplest end-to-end case, then middle steps and special cases.
2. **Operations**: "manage", "handle", "support" hide CRUD; one operation per bead.
3. **Rule or data variations**: one rule or data shape per bead.
4. **Interface method**: plainest interface first (`--json` before a table).
5. **Major effort**: the bead carrying the cost, then the trivial additions.
6. **Simple / complex**: "what is the simplest version?" as its own bead.
7. **Defer performance**: slow correct version first.
8. **Spike**: last resort.

Horizontal cuts (schema / API / UI) only when each layer is a different lane and the edges are
recorded; otherwise two workers meet in one file. The cut by **disjoint file sets** is the most
effective mechanism adopter measured (86% of files touched by one branch; 7 files ever
conflicted across 63 merges, `overnight-fleet-retrospective.md:326-331`).

### 4. Write each child (INVEST + the four triage requirements)

Before filing, every answer is yes:

- **Independent**: lands in any order within its wave, or the order is a `blocks` edge.
- **Valuable**: moves "Done when" closer or retires a named risk.
- **Small**: one session, one diff (table above).
- **Testable**: acceptance is one command or test name runnable inside the worktree by the
  agent. Not prose; not a thing only the owner can do (12 of 49 adopter beads had acceptance
  no agent could reach, `task-specification-research.md:526-545`).
- **Lane named** on the bead, and **checked against the acceptance, not the description**: two
  adopter children had prose that respected a boundary and acceptances that both required the
  same edit (`bead-dedup-audit-2026-08-17.md:48-60`). "Only the acceptance decides when a bead
  closes."
- **Citations** (`file:line`) open and match now.
- Description and acceptance agree; a contradiction gets `bd human`, not a guess.
- `owner` / `human` / `runtime` labels applied at filing; they are withheld from `next`.

```bash
echo "<what and why; cites file:line>" | bd create "<verb-first title>" --type=task -p 2 \
  --parent <epic> --description=- \
  --acceptance "cargo nextest run -p air-ledger claims::" \
  -l lane:crates/ledger --estimate 45
bd create "Spike: <question>" --type=spike --parent <epic> -p 1 \
  --acceptance "bd comment on this bead names the chosen approach and the rejected ones, with reasons"
# order and shared files: edges between CHILDREN, never on the epic
# (an edge on an epic propagates to every child; adopter ad-24r0)
bd dep add <later> <earlier>
bd dep cycles
bd dep tree <epic> --json
```

`bd create --validate` stays on. Note: in bd 1.2.1 it checks required description sections
from the beads config, not `--acceptance`; Air's triage check is what refuses an empty
acceptance.

### 5. Stop at one wave

File the skeleton plus the children that can start once it lands (3 to 6 beads for three
workers). Everything else is a line under "Later" or a capture. Metis: "Do NOT create large
numbers of tasks without human review"; Patton: "mile wide, inch deep". Re-run 2-4 when the
frontier thins.

### 6. Re-cut on evidence (judgement; Air only flags)

- A spike closes → cut its dependents now, from the finding.
- A landed sibling already satisfies a child's acceptance → close it with the evidence (six of
  ten beads on one adopter lane were already fixed on `main`; checking took 20 minutes,
  `overnight-fleet-retrospective.md:509-517`).
- A landed sibling moved a cited file → `air next` prints "stale since <sha>"; re-verify or
  re-file with `supersedes`.
- A capture says the approach is wrong → stop filing; tighten `--design` or close the epic with a
  successor (Metis feature-creep rule, third branch).
- Small and related discovery → a child under this epic. Significant → capture. Scope change →
  stop (Metis `feature-development.md:138-157`).

## Cutting per-worker queues (owner 2026-08-20: beads fields only)

Queues are `assignee` + priority + `blocks` edges. No Air-side queue. Four reads:

```bash
air status                                        # lanes held; idle / stuck per session
bd ready --parent <epic> --unassigned -n 0 --json # the frontier (never the capped default)
bd list --status in_progress --json               # what each worker holds
bd dep tree <epic> --json                         # what each landing unblocks
```

1. **One live claim per lane.** For each idle worker, the highest-priority ready bead whose lane
   no in-progress bead holds. `bd update <id> --assignee <worker>`; the worker's
   `bd update --claim` is the fact, the assignee is the suggestion.
2. **Depth ≤ 2 per worker**, as a counter not a gate (Metis "max 2 active per person"; owner
   2026-08-18 item 5). The second item is what becomes ready when the first lands.
3. **Priority encodes the wave**: skeleton `-p 1`, its direct dependents `-p 2`, the rest
   `-p 3`; `bd ready --sort priority` orders the frontier with no extra state.
4. **Shared file → edge, not assignment.** An edge survives a worker swap; an assignment does not.
5. **Starvation**: fewer ready beads than idle workers → run steps 2-5 of the procedure first.
6. **Stuck worker** (`air status`): release the bead (`bd update <id> --status open`); never
   reassign while claimed.

## Quality checklist and smells (Metis, with Air's additions)

Good child: independently valuable, clearly scoped, right-sized, aligned to the epic.
Smells: too granular ("write line 42"), too vague ("make it better"), wrong level, orphaned,
overlapping. Metis's anti-pattern names worth using in reviews: shadow backlog, orphaned work,
premature decomposition, wrong granularity, metric gaming ("splitting work unnecessarily to
increase completion count").

Air additions: an acceptance that cannot be written as one runnable condition is not ready to
file; a checklist description is an uncut epic; `--design` empty on an epic is a defect (zero of
49 adopter beads used it, `task-specification-research.md:584-594`); `bd ready` output must
be read with `-n 0` (the default cap of 100 produced a wrong conclusion about five beads,
`bead-dedup-audit-2026-08-17.md:134-144`).

## Judgment calls (Metis)

- Uncertain scope? Spike first, then decompose from the finding.
- Large epic? Probably several capability increments.
- Tiny epic? Probably a bead.
- Cross-cutting? Beads under several epics, or a dedicated platform epic.

## Additional resources

- `references/decomposition-patterns.md`: Metis's pattern catalog (verbatim, vocabulary note on top).
- `docs/research/metis-decomposition-and-agile.md`: sources for every rule above, the agile
  mapping, adopter evidence, and the dialectic on ceremony.
- `phase-transitions` for when an epic or bead may move; `scoping-workstreams` for the
  read → explore → design → plan → file → graph → self-check loop; `beads` for `bd` quoting and traps.

## Provenance

- Source: `~/projects/metis/plugins/metis/skills/decomposition/SKILL.md` and
  `references/decomposition-patterns.md` (metis `6745810`). v1.1.0 also draws on
  `plugins/metis/skills/project-patterns/references/{feature-development,anti-patterns,core-principles}.md`
  and `.metis/adrs/METIS-A-0003.md` (same SHA).
- Ported 2026-08-18; extended 2026-08-20.
- Adaptations (v1.0.0): Metis vocabulary (Vision/Initiative/Task; initiative phases
  discovery→design→ready→decompose→active→completed) remapped to Air's (feature/epic/bead; epic
  path with the triage commitment point; bead step machine from `docs/research/SYNTHESIS.md §4.2`),
  with a vocabulary map marking what is Metis's and what is Air's; "Air addition" paragraphs for
  the triage commitment point, `bd dep` edges, acceptance lines, and a "Doing it in bd" section;
  the reference file copied verbatim with a four-line note prepended.
- Adaptations (v1.1.0): added the coordinator procedure (the `--design` heading template mapped
  to Metis exit criteria; walking skeleton from Patton/Cockburn; Lawrence's splitting-pattern
  order and selection rule; INVEST-shaped child checklist; one-wave rule; re-cut rules from
  Metis's feature-creep rule), Air's sizing unit with adopter's floor and `--estimate`,
  per-worker queue cutting per `docs/decisions.md` 2026-08-20, and adopter evidence lines
  (file:line). Metis reasoning kept and labelled. Full sources and access dates:
  `docs/research/metis-decomposition-and-agile.md §13`.
