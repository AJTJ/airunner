---
name: decomposition
description: Use when asked to "break down this feature", "decompose this epic", "create beads from an epic", "how to size beads", "when to decompose", "vertical slices", "task granularity", or when an epic is about to be opened for claiming and needs children. Guides breaking a feature into epics and epics into claimable beads, in Air's vocabulary, using Metis's decomposition reasoning.
metadata:
  version: 1.0.0
---

# Work Decomposition

This skill guides breaking higher-level work into actionable lower-level items. The reasoning
is Metis's (`decomposition` skill); the vocabulary and the phase model are remapped to Air's.
Where the text below says "epic" or "bead", Metis says "initiative" or "task"; the map is
explicit so you can tell what is Metis's and what is Air's.

## Vocabulary map (Metis → Air)

| Metis | Air | Notes (Air's, not Metis's) |
|---|---|---|
| Vision | **feature** | Which feature to shoot for stays with the owner; it is not automated (`docs/decisions.md`, 2026-08-17). |
| Initiative | **epic** | A `bd` epic (`bd create … --type=epic`); children attached with `--parent`. Non-empty `--design` before an epic can be claimed (SYNTHESIS §4.3 check 8). |
| Task | **bead** | Sized to one agent, one session, one reviewable diff (`beads` skill). |
| Initiative phase `discovery → design → ready → decompose → active → completed` | epic path `discovery → design → decompose → triaged → active → closed` | Air folds Metis's `ready` into the **triage commitment point**: a bead is not `ready` until acceptance + labels + edges exist (SYNTHESIS §4.3 check 4). See the `phase-transitions` skill for the full remap. |
| Task phase `backlog → todo → active → completed` | bead step machine `triaged → claimable → claimed → working → verified → awaiting_review → landed → closed` (+ `blocked`/`stale`/`escalated`) | SYNTHESIS §4.2; `docs/plans/0001-first-slice.md §4` for the hand-over gate. |
| "decompose phase is a visible buffer" | same | Air makes it measurable: time from epic design to first child claim is a ledger-derived metric. |

## The decomposition chain (Metis, remapped)

```
Feature:  "Make X a better experience"            (owner picks)
    ↓
Epic:     "Reduce page load time by 50%"          (bd epic, with --design)
    ↓
Beads:    "Profile slow queries", "Add caching layer", "Optimize images"
```

Each level breaks the work above it into concrete, actionable pieces at the appropriate scope.

## When to decompose (Metis)

Decompose **ahead of capacity**, not upfront:

- When the current `bd ready` list is nearing its end.
- During the tail end of current work, to prepare the next batch.
- When the backlog is getting low (the signal to look up and pull work down).

**Avoid** decomposing everything upfront (waterfall). Have work ready when capacity frees up, not
the entire project planned before starting.

## The decompose phase (Metis, remapped)

Epics have an explicit decompose phase:

```
discovery → design → decompose → triaged → active → closed
```

### Why decompose is explicit (Metis)

The decompose phase creates a **visible buffer**:

- Designed epics can pile up waiting to be broken into beads.
- It tracks how long things sit there.
- It makes bottlenecks visible when several agents share a queue.

**Do not skip to decompose early.** Premature decomposition leads to beads that solve the wrong
problem, rework when the design changes, and wasted effort.

**Air addition:** the exit of decompose is the triage commitment point. Every child bead has an
`--acceptance` written as one observable condition checkable inside the worktree, the labels
that decide who can finish it (`owner`, `runtime`, lane), and its `bd dep` edges. Until then the
epic's children are not `ready` and no agent is offered them.

## Sizing by scope, not time (Metis)

Size by scope and impact, not implementation time.

### Beads: atomic units

- **Scope**: a discrete, completable piece with clear done criteria (in Air: the acceptance
  line).
- **Impact**: moves the needle on the parent epic.
- **Independence**: can be worked without constant coordination.
- Examples: "Add caching layer", "Write migration script", "Update API endpoint".

**If a bead has meaningful sub-parts**, it is probably an epic.

### Epics: capability increments

- **Scope**: creates a fundamental increment in capability.
- **Impact**: meaningfully changes what the system can do.
- **Coherence**: beads within it work toward one outcome.
- Examples: "User authentication", "Search functionality", "Billing integration".

**If it does not change what the system can do**, it may just be a bead.

## Decomposition patterns (Metis)

### Vertical slices (preferred)

Break by user-visible functionality:

```
Epic: "User authentication"
├── Bead: "Login flow"
├── Bead: "Registration flow"
├── Bead: "Password reset"
└── Bead: "Session management"
```

Each bead delivers something a user can see or use.

### Horizontal layers (use sparingly)

Break by technical component:

```
Epic: "User authentication"
├── Bead: "Database schema"
├── Bead: "API endpoints"
├── Bead: "Frontend components"
└── Bead: "Integration tests"
```

Creates dependencies between beads. Prefer vertical slices. (Air: if you do this, record every
edge with `bd dep add`, or two agents will meet in the same file.)

### Risk-first

Break by unknowns:

```
Epic: "ML recommendation engine"
├── Bead: "Spike: evaluate model options" (high uncertainty)
├── Bead: "Build training pipeline" (after spike)
└── Bead: "Integration with product" (low uncertainty)
```

Address risky work first to fail fast.

### Milestone-based

Break by deliverable checkpoints:

```
Epic: "Platform migration"
├── Bead: "Phase 1: read path on new platform"
├── Bead: "Phase 2: write path on new platform"
├── Bead: "Phase 3: deprecate old platform"
└── Bead: "Phase 4: cleanup"
```

Each milestone independently valuable and deployable.

## Quality checklist (Metis)

Good decomposition; each child:

- **Independently valuable**: delivers something useful alone.
- **Clearly scoped**: you know when it is done.
- **Right-sized**: matches scope expectations for its level.
- **Aligned to parent**: clearly contributes to the level above.

Bad decomposition smells:

- **Too granular**: "write line 42" is a step, not a bead.
- **Too vague**: "make it better" has no completion criteria.
- **Wrong level**: does not match the scope of its type.
- **Orphaned**: does not trace back to a parent.
- **Overlapping**: multiple items covering the same ground.

**Air additions:** a bead whose acceptance cannot be written as one observable condition is not
ready to file (`beads` skill); a bead with a checklist in its description is an epic that was not
cut (`bd ready` cannot see or claim checklist items).

## Common mistakes (Metis)

| Mistake | Problem | Fix |
|---|---|---|
| Decomposing too early | Beads solve the wrong problem | Stay in discovery/design until the approach is clear |
| Decomposing too late | Epic active with no beads | Decompose before moving to active |
| Wrong granularity | Beads that are epics or vice versa | Apply the scope heuristics |
| Missing alignment | Beads do not contribute to the epic | Each bead needs an obvious connection to its parent |

## Judgment calls (Metis)

- **Uncertain scope?** Create a spike bead first, then decompose based on findings.
- **Large epic?** Consider whether it is really multiple capability increments.
- **Tiny epic?** Consider whether it is really just a bead.
- **Cross-cutting?** May need beads under multiple epics, or a dedicated "platform" epic.

## Doing it in bd (Air)

```bash
# epic with design (--design=- does not read stdin; use --design-file - or inline; see beads skill)
bd create "Epic title" --type=epic -p 2 --description "Why this epic exists" \
  --design-file - < design.md

# children
echo "What and why" | bd create "Child title" --type=task -p 2 --parent <epic-id> \
  --description=- --acceptance "One observable condition" -l <lane>

# edges between children, then check the graph
bd dep add <child-b> <child-a>
bd dep cycles
bd dep tree <epic-id> --json
```

See `scoping-workstreams` for the full read → explore → design → plan → file → graph → self-check
loop around an epic, and `phase-transitions` for when an epic or bead may move.

## Additional resources

- `references/decomposition-patterns.md`: Metis's complete pattern catalog with examples
  (verbatim, with a vocabulary note at the top).

## Provenance

- Source: `~/projects/metis/plugins/metis/skills/decomposition/SKILL.md` and
  `references/decomposition-patterns.md` (metis `6745810`).
- Ported 2026-08-18.
- Adaptations: Metis vocabulary (Vision/Initiative/Task; initiative phases
  discovery→design→ready→decompose→active→completed) remapped to Air's (feature/epic/bead; epic
  path discovery→design→decompose→triaged→active→closed with the triage commitment point;
  bead step machine from `docs/research/SYNTHESIS.md §4.2`), with a vocabulary map table marking
  what is Metis's and what is Air's; "Air addition" paragraphs added for the triage commitment
  point, `bd dep` edges, acceptance lines, and a "Doing it in bd" section; the reference file
  copied verbatim with a four-line note prepended. Metis's reasoning otherwise unchanged.
