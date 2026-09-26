---
name: air-decomposition
description: Use when asked to "break down this feature", "decompose this epic", "create beads from an epic", "how to size beads", "when to decompose", "vertical slices", "task granularity", "cut the next wave", or when an epic is about to be opened for claiming and needs children. Guides breaking a feature into epics and epics into claimable beads, in Air's vocabulary, using Metis's decomposition reasoning plus the agile splitting rules behind it (INVEST, story splitting, walking skeleton).
metadata:
  version: 1.2.0
---

# Work Decomposition

Breaking a feature into epics and an epic into claimable beads that workers pull. The reasoning is Metis's (its decomposition skill, Flight Levels as Kanban) plus
the agile sources it draws on; the vocabulary, the checks, and the `bd` commands are Air's.
Where the text says "epic" or "bead", Metis says "initiative" or "task". Sources are listed
under "Additional resources".

## Vocabulary map (Metis → Air)

| Metis | Air | Notes (Air's, not Metis's) |
|---|---|---|
| Vision | **feature** | Owner picks it; not tracked (owner, 2026-08-17). A paragraph in the epic's `--design`. |
| Initiative | **epic** | `bd create --type=epic` with `--design` as its spec. Non-empty `--design` before any child can be claimed (SYNTHESIS §4.3 check 8; owner 2026-08-18 item 1). |
| Task | **bead** | One agent, one session, one reviewable diff, one runnable acceptance. |
| Backlog item (bug/feature/tech-debt) | **capture** | One line, not `ready`, no acceptance. Workers capture; they never file (`docs/design.md` §8.1). |
| Initiative `discovery → design → ready → decompose → active → completed` | epic `discovery → design → decompose → triaged → active → closed`, **derived, never stored** | Metis's `ready` is Air's triage commitment point on the children. |
| "decompose phase is a visible buffer" | same | Ledger metric: time from `--design` to first child claim. |
| `estimated_complexity` XS-XL | `--estimate <minutes>` on beads, recorded not gated | "the missing instrument is a size estimate at filing time" (an adopter's retrospective of its overnight fleet run of 2026-08-15). |

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

- Acceptance needs "and" → split.
- Checklist in the description → an epic that was not cut (`bd ready` cannot see or claim items;
  an adopter's task-specification research, read 2026-08-20, whose phrase for them is "epics wearing task clothes").
- Touches two lanes → split by lane, or record the edge and accept serial landing.
- Worker would have to pick an approach → spike first, or decide it in the description.
- Many unanswerable questions while writing the acceptance → too uncertain; spike or capture.
  Many rules → too big. One rule with many examples → a hidden second rule
  (the same research).
- **Floor as well as ceiling**: not below one reviewable diff; "a bead too small to review as one
  change costs a full merge cycle for a trivial diff" (the same research).
- Prefer three small beads to one medium; they parallelise. No numeric ceiling on children or
  lines is set until the ledger's estimate-vs-actual metric (measurement spec §2.7) produces one.
- Set `--estimate <minutes>`; it is a guess the ledger correlates with actuals, never a gate.

## The procedure (the coordinator files and decides; the reading may be delegated)

Inputs: the feature paragraph; `main`; `bd list --type=epic`; `bd ready -n 0`; the capture
inbox; `air status` (lanes held, idle workers). Output: one epic, the skeleton plus one wave of
triaged children with edges. Never the whole feature.

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
effective mechanism the adopter measured (86% of files touched by one branch; 7 files ever
conflicted across 63 merges; an adopter's retrospective of its overnight fleet run of 2026-08-15).

### 4. Write each child (INVEST + the four triage requirements)

Before filing, every answer is yes:

- **Carries its context**: the description ends with a `## Context` section, so what the worker
  needs does not depend on it remembering to look:

  ```
  ## Context
  Skills: the project skills the worker invokes before starting
  Read: the files and design-doc sections to read first
  Update: any doc the work must also change, or "none"
  ```

  The worker that claims the bead invokes the named skills first. An adopter added this on
  2026-09-25 after finding its frontend skill rarely loaded: agents hand-built controls the
  shared component library already had, about 17 buttons and two switches that still carried a
  bug the shared one had fixed. Removed when a round shows workers loading the right skills
  without it.
- **Independent**: lands in any order within its wave, or the order is a `blocks` edge.
- **Valuable**: moves "Done when" closer or retires a named risk.
- **Small**: one session, one diff (table above).
- **Testable**: acceptance is one command or test name runnable inside the worktree by the
  agent (12 of an adopter's 49 beads had "acceptance no agent can reach",
  an adopter's task-specification research, read 2026-08-20). Where it cannot be, say so in the clause and label
  the bead `owner` — see the two questions below, which refine this bullet rather than repeat
  it: a person is a legitimate settler, an unlabelled one is the defect.
- **Lane named** on the bead, and **checked against the acceptance, not the description**: two
  of an adopter's children had prose that respected a boundary and acceptances that both required
  the same edit (an adopter's bead dedup audit of 2026-08-17). "Only the acceptance decides when a bead
  closes."
- **Citations** (`file:line`) open and match now.
- Description and acceptance agree; a contradiction goes to the coordinator (`air capture`), who files it as a bead labelled `owner` when the decision is the owner's; never a guess.
- `owner` / `human` / `runtime` labels applied at filing; `air claim` refuses an `owner` bead
  to workers.

#### The two questions, asked of the TEMPLATE before it is applied

Eleven of the adopter's 99 auto-closed beads had acceptance criteria **no closing rule could ever
have settled**: nine were browser judgements, two needed a live system or a person to record
evidence. Their finding: *"That is not a failure of the closing rule. Such a criterion reads as
rigour, passes review, and makes any rule that closes on it look correct."*

The nine were not steady drift. They were **one round and one copied template across ten
siblings**. So ask these of the template, once, before it becomes ten beads:

1. **What settles this — a command, a file state, or a PERSON?** A person is a legitimate and
   often the only true answer. Write it that way and label the bead `owner`. The defect is a
   human judgement written as a fact, because that is the one a rule will close on.
2. **If it were satisfied, would anything be DIFFERENT?** A clause can pass (1) and still be
   worthless. `+html.tsx exists` is trivially checkable and the file is inert, so satisfying it
   ships nothing.

And one level down, which caught four clauses in a single session of the adopter's: **could the
INSTRUMENT satisfy this instead of the code?** A clause the test harness, the fixture, or the
probe can make true on its own is not a clause about the work.

This is where `air land`'s unreadable-clause verdicts come from, and **the fix is at filing, not
at landing**. Air discharges a clause only by lookup and reports the rest as unreadable; it
cannot judge prose, and nothing downstream will.

**No mechanical check, deliberately.** The adopter attempted a prose regex for this and measured
it at **80% false positives** on their own queue before dropping it; the distinction here is
finer than the one that failed. If anyone proposes one again, the discriminator is the absence
of a named observer or artefact, and **the rate must be measured and reported before it is
wired in** — not after (owner, 2026-08-29).

```bash
echo "<what and why; cites file:line>" | bd create "<verb-first title>" --type=task -p 2 \
  --parent <epic> --description=- \
  --acceptance "cargo nextest run -p air-ledger claims::" \
  --estimate 45
bd create "Spike: <question>" --type=spike --parent <epic> -p 1 \
  --acceptance "bd comment on this bead names the chosen approach and the rejected ones, with reasons"
# order and shared files: edges between CHILDREN, never on the epic
# (an edge on an epic propagates to every child; the adopter)
bd dep add <later> <earlier>
bd dep cycles                      # must print none BEFORE the wave is opened
bd dep tree <epic> --json
```

**Never point a child at its own epic.** An epic finishes when its children finish, so a child
that waits on its epic waits on itself: the pair can never move and `bd dep cycles` does NOT
report it, because the epic's dependence on its children is definitional rather than an edge.
An adopter did this to four children on 2026-09-05 and every P1 in their queue was unreachable
for a night; 42 beads were offered to workers and none was a P1. The tracker rendered it as
"not ready yet", indistinguishable from ordinary queueing.

**bd will usually stop you, and the case it misses is the one you are most likely to hit.**
Measured against bd 1.2.2 on nine routes, 2026-09-06 (every command and output is kept in
Air's own repository, in its bd facts reference). bd's guard is two rules and neither
is an ancestor walk:

1. **An existing `parent-child` row on the same pair**, so any other edge type between them is
   refused. That covers the DIRECT parent, always, by every route — `bd dep add`, the
   `bd dep X --blocks Y` spelling, `--no-cycle-check`, bulk `--file`, and `--graph`.
2. **A dotted-id prefix test**, which catches deeper ancestors only when the id encodes the
   chain, as `air-80x.1.1` does under `air-80x`.

So the hole is an ancestor **two or more levels up whose id does not encode the chain**, and
**`bd create --graph` produces exactly that**: it assigns flat ids and links by `parent_key`.
Filing a wave from a graph plan is the one ordinary route that builds this deadlock, and it
prints nothing at all when it does. `bd create --parent <child> --deps <grandparent>` in a
single invocation does the same. The pair that proves it is the id shape rather than the depth:

    bd dep add air-80x.1.1 air-80x     # dotted ids:   refused, naming the deadlock
    bd dep add zz-72f zz-3yf           # flat ids:     ✓ Added dependency … (blocks)

both being a grandchild pointing at its grandparent.

So, after filing a wave and before opening it — and above all after a `--graph` file: run
`bd dep cycles` (it must print none, and it will print none for this shape, so it is the weaker
check), and read `bd dep tree <epic> --json` for an edge from a child to ANY ancestor, not just
its parent. Both are one call each and both are cheap next to a night of unreachable work.
Air names this shape in `air status` (air-btz), which is the failsafe, not the check.

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
  ten beads on one adopter's lane were already fixed on `main`; checking took 20 minutes,
  an adopter's retrospective of its overnight fleet run of 2026-08-15).
- A landed sibling moved a cited file → re-verify the citation, or re-file with `supersedes`.
- A capture says the approach is wrong → stop filing; tighten `--design` or close the epic with a
  successor (Metis feature-creep rule, third branch).
- Small and related discovery → a child under this epic. Significant → capture. Scope change →
  stop (Metis `feature-development.md:138-157`).

## Keeping workers fed (owner 2026-08-21: pull, never assign)

There are no per-worker queues. Workers pull from a legible ready list; the coordinator's job
is that the list is never empty of claimable tasks and that priority says what goes first.
**Do not set `assignee` on an open bead**: in bd 1.2.x a pencilled assignee blocks every other
worker's `--claim` (three collisions in one round, 2026-08-21). Reads:

```bash
air status                                        # sessions, claims, green, leases
bd ready --type task -n 0 --json                  # the claimable frontier (epics/spikes excluded)
bd dep tree <epic> --json                         # what each landing unblocks
```

1. **Legible list**: epics and spikes are not claimable until decomposed; a task is claimable
   when it has acceptance, an estimate, and its files named in the description.
2. **Priority encodes the wave**: skeleton `-p 1`, its direct dependents `-p 2`, the rest
   `-p 3`; `bd ready --sort priority` orders the frontier with no extra state.
3. **Shared file → edge, never assignment.** `bd dep add` survives a worker swap.
4. **No depth cap** ("there is no cap; we set our goals and finish them").
5. **Starvation** (fewer ready tasks than workers): decompose next; the reading may be
   delegated to any agent with a file deliverable, the filing and deciding stay here.
6. **Stuck or gone worker** (`air status`, the channel): `air reclaim <id> --worker <name>
   --reason reassigned`; never edit assignee.

## Quality checklist and smells (Metis, with Air's additions)

Good child: independently valuable, clearly scoped, right-sized, aligned to the epic.
Smells: too granular ("write line 42"), too vague ("make it better"), wrong level, orphaned,
overlapping. Metis's anti-pattern names worth using in reviews: shadow backlog, orphaned work,
premature decomposition, wrong granularity, metric gaming ("splitting work unnecessarily to
increase completion count").

Air additions: an acceptance that cannot be written as one runnable condition is not ready to
file; a checklist description is an uncut epic; `--design` empty on an epic is a defect (zero of
49 of an adopter's beads used it, an adopter's task-specification research, read 2026-08-20); `bd ready` output must be read with `-n 0`
(the default cap of 100 produced a wrong conclusion about five beads, an adopter's bead dedup audit of 2026-08-17).

## Judgment calls (Metis)

- Uncertain scope? Spike first, then decompose from the finding.
- Large epic? Probably several capability increments.
- Tiny epic? Probably a bead.
- Cross-cutting? Beads under several epics, or a dedicated platform epic.

## Additional resources

- `references/decomposition-patterns.md`: Metis's pattern catalog (verbatim, vocabulary note on top).
- Agile sources, all accessed 2026-08-20: INVEST and SMART (Wake 2003,
  https://xp123.com/invest-in-good-stories-and-smart-tasks/); the splitting patterns and their
  selection rule (Lawrence and Green,
  https://www.humanizingwork.com/the-humanizing-work-guide-to-splitting-user-stories/); the
  walking skeleton (Cockburn as quoted at
  https://gojko.net/2014/06/09/forget-the-walking-skeleton-put-it-on-crutches/; Patton,
  https://www.jpattonassociates.com/wp-content/uploads/2015/03/story_mapping.pdf); "the
  Developers who will be doing the work are responsible for the sizing" (Scrum Guide 2020,
  https://scrumguides.org/scrum-guide.html). Metis paths are at metis `6745810`.
## Provenance

- Source: `metis/plugins/metis/skills/decomposition/SKILL.md` and
  `references/decomposition-patterns.md` (metis `6745810`). v1.1.0 also draws on
  `plugins/metis/skills/project-patterns/references/{feature-development,anti-patterns,core-principles}.md`
  and `.metis/adrs/METIS-A-0003.md` (same SHA).
- Ported 2026-08-18; extended 2026-08-20.
- Adaptations (v1.0.0): Metis vocabulary (Vision/Initiative/Task; initiative phases
  discovery→design→ready→decompose→active→completed) remapped to Air's (feature/epic/bead; epic
  path with the triage commitment point; the bead step machine from Air's 2026-08-20 research),
  with a vocabulary map marking what is Metis's and what is Air's; "Air addition" paragraphs for
  the triage commitment point, `bd dep` edges, acceptance lines, and a "Doing it in bd" section;
  the reference file copied verbatim with a four-line note prepended.
- Adaptations (v1.1.0): added the coordinator procedure (the `--design` heading template mapped
  to Metis exit criteria; walking skeleton from Patton/Cockburn; Lawrence's splitting-pattern
  order and selection rule; INVEST-shaped child checklist; one-wave rule; re-cut rules from
  Metis's feature-creep rule), Air's sizing unit with the adopter's floor and `--estimate`,
  per-worker queue cutting (owner, 2026-08-20), and the adopter's evidence lines
  Metis reasoning kept and labelled. Full sources and access dates: "Additional resources".
- An adopter's evidence is cited by what it is and its date, not by path (owner, 2026-09-25):
  the notes first cited here by `file:line` (read 2026-08-20, re-checked 2026-08-22, air-xsj)
  were consolidated by the adopter and no longer exist at those paths.
- Adaptations (v1.2.0, 2026-09-25, air-vuwx): brought into line with `.air/roles.md`, which
  holds the fleet protocol since the owner's ruling of that day. Per-worker queues and
  assignments are gone (workers pull; naming a bead reserves nothing), and so is a `next`
  subcommand that was planned and never built.
