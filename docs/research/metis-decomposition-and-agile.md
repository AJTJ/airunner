# Metis decomposition and phases, the agile behind them, and what Air's coordinator runs

**Question:** how should Air's coordinator cut a feature into epics and beads and move them
through phases, stealing from Metis's decomposition and phase-transition reasoning and from the
agile methodology Metis draws on. Deliverable: material the coordinator runs, not a survey.

**Accessed:** 2026-08-20. Metis paths are relative to `~/projects/metis` at `6745810`.
The adopter paths are relative to `the adopter's checkout` at `71191e0` (read-only). Air paths are
relative to this repo. Anything marked *(inference)* is my reading, not something a source says.

Extends, does not repeat: `metis-deep-dive.md` (what Metis is; §4 borrow list; §5 gaps),
`plans/0002-what-to-work-on.md` (Air's feature → epic → bead model and the triage commitment
point), the ported `decomposition` and `phase-transitions` skills, `decisions.md` 2026-08-18
and 2026-08-20.

---

## 0. Protocol (written before reading anything)

Sub-questions: (1) which agile concepts Metis encodes and where; (2) what the agile primary
sources say; (3) what Air already has; (4) what decomposition did well or badly in the live
The adopter's fleet; (5) steal / adapt / reject; (6) where ceremony is overhead for one coordinator
plus three workers.

Priors: Metis is a Flight-Levels/Kanban shape (hierarchy + phases + pull), not Scrum; INVEST
and vertical slicing map cleanly onto bead sizing; sprints, points, and stand-ups will not earn
their keep. Falsifiers: Metis source carrying sprint or estimation machinery that the adopter
record says was missed; retrospectives showing failures that need ceremony rather than a check.

Inclusion: Metis source, Air docs, the adopter's notes, primary agile texts (Scrum Guide, Kanban
Guide, Leopold/Kaltenecker, Patton, Wake, Cockburn, Lawrence). Secondary summaries only where
the original is unreachable, and marked as such.

---

## 1. What Metis does (with `path:line`)

| Mechanism | Where | What it says |
|---|---|---|
| Three-level hierarchy, named methodology | `crates/metis-docs-mcp/instructions.md:7`; `plugins/metis/skills/project-patterns/references/core-principles.md:3-11` | "Metis organizes work hierarchically using Flight Levels methodology: Vision (strategic) -> Initiative (projects) -> Task (work items). Work flows down through phases; feedback flows up." "Metis implements this as a Kanban system." Vision = FL3, Initiative = FL1, Task = FL0 (`plugins/metis/agents/flight-levels.md:50-54`). |
| Time horizons per level | `core-principles.md:81-92`; `instructions.md:13-15` | Vision 6 mo to 2 yr; Initiative 1 to 6 mo; Task 1 to 14 days. "A 'task' taking months is really an initiative." |
| Initiative phases | `crates/metis-docs-core/src/domain/documents/types.rs:199-206` | `discovery → design → ready → decompose → active → completed`, forward-only, adjacent-only; `next_phase` is the first valid transition (`types.rs:236-238`). |
| Task phases with one parking state | `types.rs:207-213` | `backlog → todo → {active, blocked}`; `active → {completed, blocked}`; `blocked → {todo, active}`. The only backward edges are out of `blocked`. |
| Exit criteria | `.metis/adrs/METIS-A-0003.md:31-62`; `plugins/metis/skills/phase-transitions/references/phase-flow.md:69-106` | Checkbox list, max 7 per doc, "specific and objectively verifiable", "cannot be removed once defined (only refined)", all checked before spawning children. Named patterns per transition (problem validated; approach documented, risks identified, dependencies mapped; design reviewed; tasks with acceptance criteria and "task backlog is sufficient to start"; acceptance met, verified, no known defects). |
| Exit criteria are not enforced | `metis-deep-dive.md §1.4` (citing `initiative/mod.rs:419-424`, `task/mod.rs:388-393`, `transition_phase.rs:30`) | `exit_criteria_met()` returns a hard-coded `false`; `force` is never read. |
| The `ready` phase | `docs/reference/phase-lifecycle.md:44`; `docs/explanation/flight-levels.md:96`; `phase-flow.md:92-95` | "Design reviewed, ready to decompose"; exit: "Design reviewed and approved; Team capacity available; No blocking dependencies." |
| The `decompose` phase and its timing | `docs/explanation/flight-levels.md:31`; `instructions.md:205-208`; `plugins/metis/skills/decomposition/SKILL.md` (ported verbatim into Air's skill) | "Decomposition happens after design is complete, ensuring tasks are well-defined before work begins." Decompose is a visible buffer; decompose ahead of capacity, not upfront. |
| Parent gate enforced in code | `core-principles.md:184`; `metis-deep-dive.md §1.4` | `reassign_parent` refuses unless the initiative is in `decompose` or `active`; task creation does not enforce it. |
| Pull-based flow and WIP | `core-principles.md:62-79`; `plugins/metis/skills/project-patterns/references/anti-patterns.md:49-69` | "Work is PULLED, never pushed." "Task backlog low? Look to the Initiative for what to decompose next." Anti-pattern "Too Many Active Items", fix: "Limit WIP explicitly (e.g., max 2 active tasks per person)", "Finish before starting". |
| Backlog categories | `crates/metis-docs-core/src/application/services/workspace/reassignment.rs:24-28`; `core-principles.md:42-60` | `Bug`, `Feature`, `TechDebt`; backlog items are "entry points waiting to be pulled into initiatives"; a growing backlog that never gets pulled is "a smell". |
| Complexity estimate | `crates/metis-docs-core/src/domain/documents/initiative/mod.rs:13-19`; `.metis/adrs/METIS-A-0004.md:99-107` | Initiative `estimated_complexity: XS/S/M/L/XL`. ADR-004 proposed task `estimated_hours` and `assignee`; neither is in the shipped task frontmatter (`domain/documents/task/frontmatter.yaml:1-16`). |
| Feature sizing table | `plugins/metis/skills/project-patterns/references/feature-development.md:126-136` | Small = task, "single change, no design needed"; Medium = initiative, "multiple tasks, needs discovery/design"; Large = multiple initiatives. |
| Feature creep rule | `feature-development.md:138-157` | Small and related: add a task to the current initiative. Significant: new backlog item, evaluate later. Changes scope: stop, adjust the design, or split the initiative. |
| Human-in-the-loop | `instructions.md:236-277` | "ALWAYS pause and consult the human before" transitioning an initiative, design decisions, decomposing; "Do NOT create large numbers of tasks without human review." Prose only. |
| Decompose mode of the Ralph loop | `plugins/metis/hooks/stop-hook.sh:213-225` | The loop re-prompts "Continue creating tasks to fully decompose the initiative... Do NOT transition to active (user will review and approve)". Decomposition is a job with its own loop and a human exit. |
| Ralph-loop fit | `docs/explanation/ralph-loops.md:127-143` | Good fit: "Tasks with objective, testable acceptance criteria"; poor fit: "Tasks requiring human judgment", "Exploratory work with unclear scope". "Keep tasks small (2-3 iterations ideal, not 15)." |
| Acceptance criteria in the task template | `domain/documents/task/content.md:47-51` | "## Acceptance Criteria [REQUIRED]" as three checkbox placeholders. A "Definition of Done" heading appears only in a test's custom template (`application/services/document/creation.rs:907`); it is not a shipped concept. |
| Anti-patterns catalogue | `anti-patterns.md:5-195` | Shadow work, shadow backlogs, too many active items, orphaned work, skipping phases, premature decomposition, stale work, wrong granularity, metric gaming. |

What Metis does **not** have (confirming `metis-deep-dive.md §5`): no claim/assignee, no dependency
edges beyond a `blocked_by` list nothing computes over, no verification before `completed`, no
queue. Its decomposition output is documents; ordering and parallelism are left to the human.

---

## 2. The agile behind each Metis mechanism

Labels: **C** = confirmatory (tested a stated prior), **E** = exploratory (found while reading).

| Metis mechanism | Agile concept | Primary source | Label |
|---|---|---|---|
| Vision / Initiative / Task as altitudes; "work flows down, feedback flows up" | **Flight Levels** (Leopold). FL1 "belongs to the teams that do the daily work"; FL2 "zoom out from the individual team and visualise the value stream"; FL3 strategy. Explicitly "neither an organisational model nor a maturity model, nor is it a hierarchy" and "method-agnostic". | Kaltenecker and Leopold, *Flight Levels: A Short Introduction* (PDF, https://flowsphere.ch/wp-content/uploads/2024/02/Flight-Levels-A-Short-Introduction.pdf, pp. on "3.1 Flight Level 1", "3.2", "3.3", and the inventor's request not to use it as an org chart). Metis itself says it uses "three practical levels rather than the full Flight Levels framework" (`docs/explanation/flight-levels.md:119`). | C |
| "Create focus": limit concurrent work; "Only when something is finished do we start something new" | Flight Levels activity 2 of 5 (visualise, create focus, agile interactions, measure progress, operate improvements) | same PDF, §2.2: "Starting work costs money. Finishing work makes money." | E |
| Pull, WIP, phases as columns | **Kanban**: "Defining and visualizing a workflow", "Actively managing items in a workflow", "Improving a workflow"; WIP = "the number of work items started but not finished"; a Definition of Workflow needs start/finish points, WIP control, explicit policies, a service level expectation | *The Kanban Guide* v2025.5, https://kanbanguides.org/the-kanban-guide/ (CC BY-SA 4.0). Kanban University's official guide lists "Make policies explicit" and limit WIP among its practices (https://kanban.university/kanban-guide/). | C |
| "max 2 active tasks per person", "Finish before starting" | Kanban WIP limit; "stop starting, start finishing" (Anderson's phrase, widely quoted; primary text is his 2010 book, not online) | secondary confirmation only: https://www.planview.com/resources/articles/wip-limits/ | C |
| Task exit: "Acceptance criteria met; verified/tested; no known defects" | **Definition of Done**: "A formal description of the state of the Increment when it meets the quality measures required for the product." | *The 2020 Scrum Guide*, https://scrumguides.org/scrum-guide.html | C |
| `ready` phase; "Task backlog is sufficient to start" | **Ready for selection** (refinement): "Product Backlog items that can be Done by the Scrum Team within one Sprint are deemed ready for selection in a Sprint Planning event." And sizing by the doers: "The Developers who will be doing the work are responsible for the sizing." | Scrum Guide 2020, same URL | C |
| Task quality checklist: independently valuable, clearly scoped, right-sized, aligned | **INVEST** (Independent, Negotiable, Valuable, Estimable, Small, Testable); tasks are **SMART** (Specific, Measurable, Achievable, Relevant, Time-boxed). Vertical over horizontal: slice "through layers... rather than horizontal completion of single layers". | Wake, "INVEST in Good Stories, and SMART Tasks", 2003-08-17, https://xp123.com/invest-in-good-stories-and-smart-tasks/ | C |
| Vertical slices preferred; horizontal layers sparingly | **Vertical slicing** (Wake, above); **story splitting patterns**: workflow steps ("build the simple end-to-end case first and then add the middle steps and special cases"), operations ("'manage'... is a giveaway"), business-rule variations, data variations, data-entry methods, major effort, simple/complex ("What's the simplest version of this?"), defer performance, break out a spike ("last resort"). Selection: "Choose the split that lets you deprioritize or throw away a story" and that yields "more equally sized small stories". | Lawrence and Green, *The Humanizing Work Guide to Splitting User Stories*, https://www.humanizingwork.com/the-humanizing-work-guide-to-splitting-user-stories/ | E (Metis has the preference but none of the patterns) |
| Risk-first pattern; "Spike: evaluate model options" | **Spike** (XP) and **walking skeleton**: "A Walking Skeleton is a tiny implementation of the system that performs a small end-to-end function. It need not use the final architecture, but it should link together the main architectural components." | Cockburn, *Crystal Clear*, as quoted at https://gojko.net/2014/06/09/forget-the-walking-skeleton-put-it-on-crutches/ (book text not online) | E |
| Milestone pattern ("read path, write path, deprecate, cleanup") | **Story mapping release slices**: "The smallest number of tasks that allow your specific target users to reach their goal compose a viable product release." Opening game = "functional walking skeleton"; mid game completes; end game refines. Backbone items are not prioritised against each other ("We'll never release a car without brakes"). | Patton, *Story Map Concepts* (PDF, https://www.jpattonassociates.com/wp-content/uploads/2015/03/story_mapping.pdf, "Release Slice", "Slice Out a Development Strategy"); Patton, "The New User Story Backlog is a Map", 2008-10-08, https://www.jpattonassociates.com/the-new-backlog/ | E |
| "Do NOT create large numbers of tasks without human review"; "Decompose ahead of capacity, not upfront" | Story mapping's "mile wide, inch deep" backbone first; Kanban pull. Patton's criticism of flat backlogs as "context-free mulch". | Patton PDF p. "Frame... Think 'mile-wide, inch deep'"; Patton 2008 | E |
| Forward-only phases, "phases protect you" | Not agile canon. Closest is Kanban's "explicit policies" for moving between columns; Scrum has no phases. *(inference)* Metis's own justification is "a forcing function that ensures you've thought through the work before doing it" (`docs/explanation/flight-levels.md:101`). | | E |
| Complexity XS-XL on initiatives | T-shirt sizing (folk practice; no primary source claims it). Scrum's only rule is who sizes, not how. | | E |
| Backlog categories bug/feature/tech-debt | Kanban "work item types" / classes of service (Anderson). Not cited by Metis. | | E |

Two findings that change the picture relative to the prior:

- **Metis is Kanban plus phases, and says so.** No sprint, no points, no velocity anywhere in the
  source (grep of `plugins/`, `docs/`, `crates/` for `sprint|story point|velocity` returns
  nothing). The prior held.
- **Metis's decomposition reasoning is thinner than its phase reasoning.** It has the
  vertical/horizontal/risk/milestone catalogue and the sizing heuristics but none of the
  splitting patterns or the selection rule. The splitting patterns are the main thing worth
  adding to Air's skill that is not already there.

---

## 3. Mapping onto Air's vocabulary

| Metis | Air | Carrier in Air |
|---|---|---|
| Vision | feature | A paragraph in `docs/decisions.md` or a plan; owner-held; not tracked (`decisions.md` 2026-08-17; plan 0002 §1). |
| Initiative | epic | `bd create --type=epic` with `--design` as its spec (plan 0002 §2.1). |
| Task | bead | `bd create --type=task|bug|spike|chore --parent <epic>`. bd 1.2.1 has a native `spike` type (`bd create --help`). |
| Backlog item (bug/feature/tech-debt) | **capture** | A one-line bead that is not `ready`: no acceptance, no lane, `discovered-from` edge. Workers capture, they do not file (`decisions.md` 2026-08-18 item 3). |
| `backlog → todo` (human move) | capture → triaged | The triage commitment point (plan 0002 §2.2): acceptance + lane + parent/edges + resolving citations. |
| Initiative `discovery → design → ready → decompose → active → completed` | epic `discovery → design → decompose → triaged → active → closed` | Derived, not stored (`decisions.md` 2026-08-18 item 4): `--design` present; children exist; children triaged; a child claimed; all closed and epic check green. |
| Task `todo → active → completed`, `blocked` | bead `open → in_progress → awaiting_review → closed`, `blocked` | `bd` statuses (`beads` skill); Air's ledger carries claim, verify, landing evidence (plan 0001). |
| Exit criteria checkboxes (`exit_criteria_met`) | evidence rows | `verify_runs` at HEAD, `merge-base --is-ancestor`, `landings`; never model text (plan 0001 §4). |
| `estimated_complexity` on initiative | not stored | Sizing is a filing-time judgement (§7 below); the ledger measures actual single-session success. |
| Human approval before initiative transitions (prose) | owner picks the feature; coordinator moves epics; the one refusal is the hand-over gate | Roles per `decisions.md` 2026-08-20. |
| Ralph "decompose mode" | a coordinator session running the procedure in §6 | No loop machinery; the coordinator is a human-facing session. |
| `reassign_parent` (only into decompose/active initiatives) | `bd update <id> --parent <epic>` | Air's rule: only into an epic with non-empty `--design` (check 8). |
| "max 2 active tasks per person" | WIP counter, measured not enforced | `decisions.md` 2026-08-18 item 5. |
| Pull: "task backlog low? decompose next" | `bd ready --parent <epic>` thinning is the trigger | plan 0002 §2.1. |

---

## 4. Steal verbatim, adapt, reject

### Steal verbatim (now in the skills)

1. Decompose ahead of capacity, never the whole feature (`decomposition/SKILL.md`, already).
2. Decompose is a visible buffer; measure time in it (already; ledger metric).
3. Exit-criteria properties: observable, specific, relevant, achievable (`phase-flow.md:73-78`).
4. "Criteria cannot be removed once defined (only refined)" (ADR-003:55). Maps to: an epic's
   `--design` end-to-end check may be tightened, not dropped, once children are filed.
5. Max 7 exit criteria per document (ADR-003:41). Maps to: an epic `--design` has at most 7
   acceptance-shaped lines; a bead has exactly one.
6. Feature creep rule (`feature-development.md:138-157`): small and related → a child; significant
   → capture; changes scope → stop and re-design or split the epic.
7. The sizing table Small/Medium/Large → bead / epic / several epics (`feature-development.md:126-136`).
8. The anti-pattern names: shadow backlog, orphaned work, premature decomposition, wrong
   granularity, metric gaming ("Splitting work unnecessarily to increase completion count").
9. Ralph-fit list as the test for whether a bead is worker-shaped (`ralph-loops.md:127-137`).
10. From agile, not Metis: INVEST/SMART, the nine splitting patterns and the selection rule, the
    walking-skeleton-first release slice, "Developers who will be doing the work are responsible
    for the sizing".

### Adapt

1. **`ready` phase → the triage commitment point on children**, not a phase on the epic. Metis's
   `ready` exit is "design reviewed; capacity available; no blocking dependencies". Air already
   holds the design review in `--design` (check 8) and capacity in pull; "no blocking
   dependencies" becomes `bd ready` itself. So the epic path stays five states, derived.
2. **Exit criteria as evidence, not checkboxes.** Keep Metis's patterns per transition, render
   each as a command or a `bd` fact (§8).
3. **Human-in-the-loop prose → role split.** Owner picks the feature; the coordinator decomposes
   and moves epics; workers move beads only through the gate. Metis's "Do NOT create large
   numbers of tasks without human review" becomes a number: a first cut is at most the walking
   skeleton plus one wave, see §6 step 5.
4. **Backlog categories → labels on captures** (`bug`, `debt`, `feat`) only if the triage pass
   wants them for grouping; not a type system.
5. **Complexity XS-XL → a filing-time rule, not a field.** §7.
6. **"Task backlog is sufficient to start"** → "at least one ready child per idle worker"
   (§9), a number the coordinator reads from `bd ready`.

### Reject, with reasons

1. **A stored epic phase** (label or field). Owner tabled it (`decisions.md` 2026-08-18 item 4).
   Derivable from `--design` + children + claims + closes. Storing it invites the Metis failure:
   a flag (`exit_criteria_met`) that nothing evaluates.
2. **A fourth level** (Flight Level 2 / "strategy"). Metis removed its own `Strategy` type
   (ADR-007). One coordinator and three workers have no cross-team value stream to visualise.
3. **Sprints, velocity, points, story-point estimation.** Not in Metis, not in the record as a
   pain. The Scrum Guide's relevant sentence (doers size) is kept; the ceremony is not.
4. **Checkbox acceptance criteria (three per task).** the adopter's record says acceptance is the
   weak link when it is prose (plan 0002 §2.2 row 1). One runnable condition per bead.
5. **Ralph decompose loop** as machinery. The coordinator is a live session; a Stop-hook loop
   adds nothing until decomposition is observed to stall.
6. **Metis's "backlog that must be groomed/archived" ritual.** Air's capture inbox is triaged
   inline (`decisions.md` 2026-08-18 item 2); capture depth is a ledger metric, not a meeting.
7. **"Every piece of work should trace back to the vision" as a rule to enforce.** Keep it as the
   triage question ("which epic?"); orphan captures are allowed to exist as captures.

---

## 5. What the live the adopter's fleet says (evidence for the cut rules)

Read via a fan-out over the retrospectives, baselines, audits, and plan 0022 (file list in §13).
Numbers are the notes' own.

### What worked

- **The cut by disjoint file sets is the most effective mechanism the record measured.** Of 353
  files touched in the overnight run, 86% were touched by exactly one branch; seven files ever
  conflicted across 63 merges (`docs/notes/overnight-fleet-retrospective.md:326-331`). "What
  prevented the rest was the file split agreed in the first ten minutes, peer-to-peer, not the
  barrier" (`:341-343`). Epics partitioned by screen group (`.8-.16`, `.15.1-.10`)
  were "split by disjoint file sets so two agents never touch one file. Correct as filed"
  (`docs/notes/bead-dedup-audit-2026-08-17.md:125-126`). This is why "Lanes" is a required
  heading in `--design` (§6) and why lane is one of the four triage requirements.
- **Decomposition churn is the system working.** 80 of 152 beads in one round were epic
  children; "mostly decomposition, not redundancy" (`docs/notes/bead-admission-control.md:15-27`).
  Duplication was 1 in 152 that round and about 3% corpus-wide
  (`docs/plans/0022-agent-working-procedure.md:89-91`). A cap on beads per agent was rejected
  because it "punishes decomposition" (`bead-admission-control.md:81`).
- **Grouping orphans into epics paid, with the reason written down**: "Grouped so it can be
  worked as one thread rather than nine unrelated P2s, and so `bd ready` does not scatter them"
  (`bead-dedup-audit-2026-08-17.md:80-81`); 13 orphans into 3 epics, "Children lost: none"
  (`:78-104`).
- **Acceptance as an observable condition clears the bar that discarded 68.3% of SWE-bench**
  (`docs/notes/task-specification-research.md:484-485`), and the best-specified bead in the
  queue was written by an agent (`:513-522`).
- **Re-verifying premises at claim time**: six of ten beads on one lane were already fixed on
  `main`; 20 minutes of checking turned six re-implementations into six closes
  (`overnight-fleet-retrospective.md:509-517`). This is §6 step 6 and the citation check.

### What went badly (each maps to a rule above)

| Failure | Evidence | Rule it produces |
|---|---|---|
| Compound beads that were epics: "nine numbered items spanning client and backend", acceptance "satisfiable by deferring all nine" | `task-specification-research.md:571-581`; "five that are epics wearing task clothes" (`:24-25`) | Checklist in a description = uncut epic (§7). |
| Epics claimable from `bd ready`; non-epics with `.N` children | `task-specification-research.md:582`; `bead-dedup-audit-2026-08-17.md:107-108` | `bd ready --exclude-type epic`; type correction at triage. |
| Two children whose **acceptances** owned the same edit while their prose respected the boundary | `bead-dedup-audit-2026-08-17.md:48-60` | Lane is checked on the acceptance, not the description (§6 step 4). |
| The missing edge "nobody had seen": `` rewrites the literals nine i18n slices move; "Whichever lands second rewrites the other's work" | `bead-dedup-audit-2026-08-17.md:70`; "not findable by the agent doing the filing. They require the corpus in view" (`0022:100-102`) | Edges are cut by whoever has the epic's whole tree in view; shared file → `blocks` edge (§9 step 4). |
| "Record a dependency whenever two beads touch the same file" was prose, "NOT ENFORCED", and absent from the not-enforced inventory | `overnight-fleet-retrospective.md:546-552` | Air's check 6 (lane at claim) and the stale-citation flag. |
| 12 of 49 beads with acceptance no agent could reach; four of the five ready P1s among them: "a queue-shape defect, not a writing defect" | `task-specification-research.md:526-545` | Triage requirement 1: acceptance runnable inside the worktree by the agent. |
| Description and acceptance disagreeing: "Neither will ask" | `task-specification-research.md:550-570` | Triage requirement 4; `bd human` on contradiction. |
| `--design` empty in all 49 beads; "Usage: zero" | `task-specification-research.md:584-594` | Check 8: non-empty `--design` before a child claim. |
| Nine beads with wrong citations; one made an acceptance clause unreachable | `docs/notes/human-queue-triage.md:1081-1096` | Citations resolve at filing and at claim. |
| Acceptance literally met while the data on disk had not changed | `overnight-fleet-retrospective.md:33` | "Write it so that satisfying it *is* the work" (`task-specification-research.md:634-637`). |
| No size field, so "was five defensible is argued and not measured"; bead duration CV 1.38, median 8 min, p90 41 min | `overnight-fleet-retrospective.md:350-355,723-726` | Record a filing-time estimate so the ledger can correlate (§7). |
| `bd create` writes straight into `bd ready`: "every observation an agent has is instantly committed work" | `0022:60-62` | Capture → triage split (adopted, `decisions.md` 2026-08-18). |
| Owner-only work filed faster than it clears: 27 of 152, then 9 of 39 | `bead-admission-control.md:29-31`; `round-2026-08-15-evening-retrospective.md:86` | `owner`/`human` labels at filing; withheld from `next`. |
| `bd ready` silently caps at 100; a wrong conclusion about five beads | `bead-dedup-audit-2026-08-17.md:134-144` | Always `bd ready -n 0` or `--json` with a count in the procedure. |
| An edge placed on an epic propagated to every child (``) | `bead-dedup-audit-2026-08-17.md:99-100` | Edges between children, never on the epic. |

### Recommendations in the notes that bear on this research

- **Size floor as well as ceiling**: "Do not decompose below one session and one reviewable diff...
  a bead too small to review as one change costs a full merge cycle for a trivial diff"
  (`task-specification-research.md:757-759`). Readiness diagnostic: "many unanswerable
  questions = too uncertain to file; many rules = too big, slice it; one rule with many examples
  = a hidden second rule" (`:265`).
- **Audit acceptance before enriching descriptions**: "a wrong or over-tight success condition
  broke twice as many tasks as a thin description did" (`:85-92`).
- **Who triages.** 0022 argues for "a dispatched agent, on a cadence, holding no lane. Not a
  hook... It is also the one job the coordinator should not do inline" (`0022:145-152`). The
  owner chose inline for now (`decisions.md` 2026-08-18 item 2). Both agree on the fallback: if
  captures outrun triage, auto-file trivial ones with `triage-needed` rather than abandon the
  split (`0022:276-280`). Flagged as an open question below, with the ledger's capture depth as
  the trigger.
- **Declined by 0022, and Air agrees**: forward-only phase enforcement ("code fails confidently...
  A phase machine will be exactly that kind of wrong the first time real work does not fit its
  model", `0022:237-239`) and the Vision → Initiative → Task hierarchy as a third tracker
  (`0022:227-231`). This is the strongest local evidence for §4 reject 1 and 2: Air keeps
  Metis's *reasoning* about phases and stores no phase.

---

## 6. The decomposition procedure (coordinator runs this)

**Inputs.** The feature paragraph (owner). The codebase at `main`. `bd list --type=epic` and
`bd ready` (what is already in flight). Capture inbox: `bd list --label capture` (or whatever
label the triage pass uses). The lanes currently held: `air holdings` / `air status`.

**Output.** One epic with a `--design`, a first wave of triaged children with edges, and a per-
worker queue (§9). Never the whole feature.

### Step 1. Frame the epic (Metis discovery + design, collapsed into `--design`)

Write `design.md` with exactly these headings; each maps to a Metis exit criterion or an Air check:

```
# <epic title>
## Why            (the named pain from the record, with citation)      [Metis discovery exit]
## Approach       (how, alternatives rejected in one line each)         [Metis design exit]
## Lanes          (files/dirs this epic owns; one line per lane)        [Air check 6]
## Out of scope   (what a worker must capture, not do)                  [feature creep rule]
## Risks          (unknowns; each becomes a spike or a blocks edge)     [risk-first]
## Done when      (ONE command that exits 0 on main when the epic is complete)  [Air epic check]
## Later          (slices deliberately not cut yet)                     [one-wave rule]
```

Rules: "Done when" is required before any child can be claimed (`decisions.md` 2026-08-18
item 1). It may be tightened later, never removed (ADR-003:55). If the whole epic fits one
sentence of diff, stop: file one bead under an existing epic (plan 0002 §2.1).

```bash
bd create "<epic title>" --type=epic -p 2 --description "<one paragraph: why>" \
  --design-file - < design.md
```

### Step 2. Find the walking skeleton (Patton / Cockburn)

Ask: what is the thinnest end-to-end path that makes "Done when" runnable, even if it returns
the wrong answer? That is child 1. It touches every layer the epic will touch, so it
discovers integration risk first (Cockburn: "link together the main architectural components").
If the skeleton cannot be written because the approach is unknown, child 1 is a
`--type=spike` with an acceptance that is a written finding, and everything else waits on it
(Lawrence: spike is the last resort; Metis: risk-first).

### Step 3. Split the rest with the pattern list, in this order

Run down the list and stop at the first pattern that produces 2-5 roughly equal children
(Lawrence's selection rule: prefer the split that lets you throw one away, then the one with
equal sizes):

1. **Workflow steps**: simple end-to-end case first, then middle steps and special cases.
2. **Operations**: "manage", "handle", "support" hide CRUD; one operation per bead.
3. **Business-rule / data variations**: one rule or data shape per bead.
4. **Data entry / interface method**: plainest interface first (`--json` before a table).
5. **Major effort**: the bead that carries the cost, then the trivial additions.
6. **Simple / complex**: "what is the simplest version?" as its own bead.
7. **Defer performance**: slow correct version first.
8. **Spike**: only when nothing above applies.

Horizontal layer cuts (schema / API / UI) are allowed only when each layer is in a different
lane and the edges are recorded; otherwise two workers meet in one file (plan 0002 §3).

### Step 4. Write each child as a bead (INVEST + Air's four triage requirements)

For each child, before filing, the coordinator can answer yes to all of:

- **Independent**: can land in any order relative to its wave, or the order is a `blocks` edge.
- **Valuable**: moves "Done when" closer, or removes a named risk.
- **Small**: one agent, one session, one reviewable diff (`beads` skill); §7.
- **Testable**: the acceptance is one command or test name runnable inside the worktree.
- **Lane**: the files it will touch are named, and no open sibling holds them (or an edge says
  who goes first).
- **Citations** (`file:line`) open and match at filing.

```bash
echo "<what and why; cites file:line>" | bd create "<verb-first title>" --type=task -p 2 \
  --parent <epic> --description=- \
  --acceptance "cargo nextest run -p air-ledger claims::" \
  -l lane:crates/ledger
# ordering and shared-file edges
bd dep add <later> <earlier>
# spikes
bd create "Spike: <question>" --type=spike --parent <epic> -p 1 \
  --acceptance "bd comment on this bead names the chosen approach and the rejected ones with reasons"
bd dep cycles
bd dep tree <epic> --json
```

`--validate` must be on (`decisions.md` 2026-08-18 item 3). Note bd 1.2.1's help says it
validates "required sections for issue type" from the repo's beads config, not the
`--acceptance` flag itself; Air's `next`/triage check is what refuses an empty acceptance.

### Step 5. Stop at one wave

File the skeleton plus at most one wave (the children that can start once the skeleton lands;
in practice 3 to 6 beads for three workers). Everything further stays as a line in
`--design` under "Later" or as a capture. This is Metis's "Do NOT create large numbers of tasks
without human review" and Patton's "mile wide, inch deep" made concrete. Re-run steps 2-4 when
`bd ready --parent <epic>` has fewer open beads than idle workers (§9).

### Step 6. Re-cut on evidence (coordinator judgement; Air only flags)

- A spike closes → decompose its dependents now, using the finding.
- A landed sibling already satisfies another child's acceptance → close it with the evidence.
- A landed sibling moved a cited file → `air next` prints "stale since <sha>"; re-verify or
  re-file with `supersedes`.
- Capture says "this epic's approach is wrong" → stop filing; edit `--design` (tighten only) or
  close the epic with a successor. (Feature creep rule, third branch.)

---

## 7. Sizing: the rule and why

**The unit is one worker session that ends with `air handover` succeeding.** Not hours, not
points. Reasons:

- Scrum's only hard sizing statement is about *who* sizes ("the Developers who will be doing
  the work"). In Air the doer is a fresh session that cannot negotiate scope mid-flight, so the
  filer has to size for it. The ledger gives the feedback loop Scrum gets from the team:
  single-agent success rate per bead (plan 0001 §8).
- Metis's "2-3 iterations ideal, not 15" (`ralph-loops.md:142`) is the same unit in Ralph terms.
- Metis's XS-XL on initiatives is never read by anything; a stored size that is not consumed
  is the `exit_criteria_met` pattern again. The adopter's retrospective names the opposite gap:
  "the missing instrument is a size estimate at filing time"
  (`overnight-fleet-retrospective.md:726`). The reconciliation: record a guess that something
  *consumes*. `bd create --estimate <minutes>` exists in 1.2.1; the ledger already has session
  start and hand-over times, so estimate vs actual per bead is a free metric. Record it, never
  gate on it (`decisions.md` 2026-08-18 item 5).

Concrete tests at filing time:

| Signal | Verdict |
|---|---|
| Acceptance needs "and" | Split (`beads` skill). |
| Description has a checklist | It is an epic that was not cut. |
| Touches more than one lane | Split by lane, or record the edge and accept serial landing. |
| Worker must choose between approaches | Spike first, or the coordinator decides in the description. |
| Diff would be reviewable in one sitting (rule of thumb under ~400 changed lines) | Right size. *(inference; no source in the record sets this number; make it a ledger observation)* |
| Three small beads or one medium | Three; they parallelise (`beads` skill). But not below one reviewable diff: "a bead too small to review as one change costs a full merge cycle for a trivial diff" (`task-specification-research.md:757-759`). |
| Many unanswerable questions while writing the acceptance | Too uncertain to file; spike or capture (`task-specification-research.md:265`). |
| Epic has more than ~12 children in total | Probably two capability increments (Metis "Large"). |

Epics: a **capability increment** with one runnable "Done when" command. If two commands are
needed, it is two epics. If "Done when" can already be made green by one bead, it is a bead.

---

## 8. Exit criteria per phase, in Air terms

Epic states are derived; each row is the evidence that the state holds.

| Transition | Metis pattern (`phase-flow.md:82-106`) | Air evidence (what the coordinator checks) |
|---|---|---|
| discovery → design | "Problem statement is clear and validated; constraints identified; stakeholders aligned on scope" | `--design` has "Why" with a citation into the record, and "Out of scope". Owner has named the feature. |
| design → decompose | "Solution approach documented; technical risks identified and mitigated; dependencies mapped" | `--design` has "Approach", "Lanes", "Risks", and a "Done when" command that currently fails (red) on `main`. |
| (Metis `ready`) | "Design reviewed and approved; team capacity available; no blocking dependencies" | Folded: `--design` non-empty (check 8) and `bd ready` computes blocking. Capacity = an idle worker. |
| decompose → triaged | "Tasks created with clear acceptance criteria; task backlog is sufficient to start; team understands the work" | Skeleton + one wave filed; every child passes the four triage requirements; `bd dep cycles` empty; `bd ready --parent <epic>` is non-empty. |
| triaged → active | (pull) | First `bd update --claim` on a child (recorded by Air on PostToolUse, `decisions.md` 2026-08-20). |
| active → closed | "Acceptance criteria met; work verified/tested; no known defects" | Every child closed or superseded (`bd epic close-eligible`) **and** `air record verify -- <Done when>` exit 0 on `main`; then `bd close <epic> --reason=-` naming shas. |

Bead transitions are unchanged from the `phase-transitions` skill (claim → working → verified →
awaiting_review via the one refusal → landed → closed).

---

## 9. Cutting per-worker queues from an epic using live state

Owner's rule: queues are beads fields only (`assignee` + priority + `blocks` edges); no Air-side
queue (`decisions.md` 2026-08-20). The coordinator builds them from four reads:

```bash
air status                                   # who holds what lane; stuck/idle per session
bd ready --parent <epic> --unassigned --json # the frontier
bd list --status in_progress --json          # what each worker holds (assignee, labels)
bd dep tree <epic> --json                    # what unblocks when each in-flight bead lands
```

Procedure, once per triage pass or whenever a worker goes idle:

1. **One live claim per lane.** For each idle worker, pick the highest-priority ready bead whose
   lane label no in-progress bead holds. Set `bd update <id> --assignee <worker>`; the worker
   claims it with `bd update --claim` when it starts (assignee is a suggestion; claim is the
   fact).
2. **Queue depth per worker ≤ 2** (Metis "max 2 active per person", as a counter not a gate).
   The second item is the bead that becomes ready when the first lands, found from `dep tree`.
3. **Priorities encode the wave.** Skeleton `-p 1`; its direct dependents `-p 2`; the rest
   `-p 3`. `bd ready --sort priority` then orders the frontier without any extra state.
4. **Shared files → edge, not assignment.** If two ready beads touch one lane, add
   `bd dep add <b> <a>` rather than assigning both to one worker; the edge survives a worker
   swap, an assignment does not.
5. **Starvation check.** If `bd ready --parent <epic>` returns fewer than the number of idle
   workers, run §6 steps 2-5 before the next pass. This is Metis's "task backlog low? look up".
6. **Stuck worker.** `air status` shows stuck; the bead is released (`bd update --status open`,
   or `--assignee ""`) and goes back on the frontier; never reassigned while claimed.

What Air measures to tune this (no gates): claims per worker, time idle between claims, time a
ready bead waits with an assignee set, stale-citation flags after a sibling lands.

---

## 10. Dialectic: the case for more ceremony, steelmanned

**Steelman.** Agile ceremonies exist because humans lose shared understanding. A fresh worker
session has *less* shared understanding than any human team member: it has never seen the
planning conversation. So the argument runs: Air needs *more* explicit ceremony, not less.
Refinement with the doers present catches bad cuts before filing; a Definition of Ready stops
under-specified beads; a retrospective after each round turns ledger numbers into rule changes;
a stored epic phase lets a new coordinator session resume without re-deriving state; and
Metis's human-approval pauses before each initiative transition are exactly the review points
a fleet with a $100/hour burn needs (Gas Town's burn, `decisions.md` 2026-08-18 "Where Gas
Town over-engineers").

**Where it holds.**

- Definition of Ready: holds, and Air has it. The triage commitment point *is* a Definition of
  Ready, enforced by a check rather than a meeting (Scrum Guide: "deemed ready for selection").
- Refinement with the doers: partly holds. The doer cannot attend, but its predecessor's
  evidence can: single-agent success per bead and stale-citation counts are the refinement
  feedback. The round retrospective notes in the adopter are the human half and cost one page.
- Resuming a coordinator session: holds as a need, but the fix is `air status` deriving the
  epic state from facts, not storing a phase a stale session could leave wrong.

**Where it fails for one coordinator and three workers.**

- Every ceremony in Scrum is sized for a team of 10 or fewer humans synchronising on a cadence.
  Here there is one human. A stand-up is `air status`. A sprint boundary would force batching
  onto a fleet whose whole advantage is continuous landing to `main` (plan 0002 §3).
- Metis's approval-before-every-transition is prose that nothing enforces, and the record says
  positive sequential rules stay prose until a check owns them (SYNTHESIS §1). Adding more
  approval points adds more unenforced prose.
- Gas Town is the measured case of process ahead of pain: "no roles, concepts, or queues ahead of
  a measured trigger" (`decisions.md` 2026-08-18). Each ceremony above would be a role or a
  queue.
- Flight Levels' own authors say the model is "neither an organisational model nor a maturity
  model, nor is it a hierarchy" and is "method-agnostic" (PDF §1, §3.1). Using it to justify a
  fourth layer or a coordination board for three worktrees is the misuse the inventor asks
  people not to make.

**Verdict.** The counter-position is right about *what* is needed (shared understanding,
readiness, feedback, resumability) and wrong about *how* at this size: each need is met by a
fact Air already records or a check it already runs, and the adopter's evidence (§5) points at
missing edges and prose acceptance, not at missing meetings. Keep the two skills as procedures
and checklists; add no ceremony until a ledger number says a specific one is missing.

---

## 11. Findings as claims

1. **Claim:** Metis's methodology is Flight Levels implemented as Kanban (hierarchy, pull, WIP,
   explicit phases); it contains no Scrum ceremony. **Type:** confirmatory. **Confidence:**
   established. **Sources:** `core-principles.md:3,62-79`; grep of the repo; Leopold PDF.
   **Counter-evidence:** none found. **Relationship to priors:** confirms.
2. **Claim:** Metis enforces only phase adjacency and the parent-phase rule on reassignment; exit
   criteria, approval, and complexity are prose or dead fields. **Type:** confirmatory.
   **Confidence:** established. **Sources:** `types.rs:188-238`; `metis-deep-dive.md §1.4`;
   `task/frontmatter.yaml`. **Relationship:** confirms and extends (complexity never consumed).
3. **Claim:** The splitting patterns (Lawrence) and the walking-skeleton release slice (Patton,
   Cockburn) are the agile content missing from Metis's decomposition skill and worth adding to
   Air's. **Type:** exploratory. **Confidence:** emerging (usefulness to be seen in the first
   epic cut with them). **Sources:** §2 table.
4. **Claim:** Metis's `ready` phase collapses into Air's triage commitment point without loss.
   **Type:** exploratory. **Confidence:** established for the definition; the three `ready`
   exit criteria each have an Air carrier (§4 adapt 1).
5. **Claim:** Sizing by "one session to a green hand-over" is the right unit, and the ledger's
   single-agent success rate is the feedback that replaces team estimation. **Type:**
   exploratory. **Confidence:** emerging. **Counter-evidence:** none in the record; no measured
   round yet.
6. **Claim:** Per-worker queues can be expressed entirely with `assignee`, priority waves, and
   `blocks` edges, and derived from four `bd`/`air` reads. **Type:** confirmatory (owner's
   decision tested against bd 1.2.1's flags). **Confidence:** established for the mechanism;
   `bd ready --parent --unassigned --sort priority` all exist (`bd ready --help`).
7. **Claim:** In the live fleet, decomposition failed on edges and acceptance (missing
   shared-file edges, unreachable or contradictory acceptance, compound beads, empty `--design`),
   not on duplication (about 3%) or on the absence of phases; and the cut by disjoint file sets
   was the single most effective mechanism measured (86% of files single-branch, 7 files ever
   conflicted). **Type:** confirmatory. **Confidence:** established (measured, §5).
   **Relationship to priors:** confirms; the ceremony falsifier did not fire.
8. **Claim:** the adopter 0022 and the owner disagree on who triages (dispatched agent on a
   cadence vs coordinator inline). **Type:** exploratory. **Confidence:** contested. **Sources:**
   `0022:145-158`; `decisions.md` 2026-08-18 item 2. Resolution proposed: inline until the
   ledger's capture depth or time-to-triage crosses a number the owner sets.

## 12. Gaps

- `bd create --validate` checks description sections from the beads config, not
  `--acceptance`; plan 0002 §2.2 row 1 reads as if it refuses empty acceptance. Either configure
  the template so the acceptance section is required, or rely on Air's triage check. Owner
  question.
- No measured round yet: the ~400-line diff heuristic and the "≤ 12 children" heuristic are
  unsourced starting points to be replaced by ledger numbers.
- Anderson's *Kanban* (2010) and Cockburn's *Crystal Clear* were cited through secondary pages;
  the book text is not online.
- Flight Levels' "measure progress" and "operate improvements" activities were not mapped; they
  correspond to plan 0001 §8 metrics and the round retrospectives, not to decomposition.

---

## 13. Sources

Metis (`~/projects/metis` at `6745810`, read 2026-08-20):
`.metis/adrs/METIS-A-0003.md`, `METIS-A-0004.md`, `METIS-A-0007.md` (via deep dive);
`crates/metis-docs-core/src/domain/documents/types.rs:188-245`;
`crates/metis-docs-core/src/domain/documents/initiative/mod.rs:13-19`;
`crates/metis-docs-core/src/domain/documents/{initiative,task}/{content.md,frontmatter.yaml}`;
`crates/metis-docs-core/src/application/services/workspace/reassignment.rs:24-28`;
`crates/metis-docs-core/src/application/services/document/creation.rs:895-915`;
`crates/metis-docs-mcp/instructions.md`;
`plugins/metis/agents/flight-levels.md`; `plugins/metis/hooks/stop-hook.sh:150-236`;
`plugins/metis/commands/metis-ralph-initiative.md`;
`plugins/metis/skills/project-patterns/references/{core-principles,anti-patterns,feature-development}.md`;
`plugins/metis/skills/phase-transitions/references/phase-flow.md`;
`docs/explanation/{flight-levels,ralph-loops}.md`; `docs/reference/phase-lifecycle.md`.

Air: `CLAUDE.md`; `docs/decisions.md` (2026-08-18, 2026-08-20); `docs/plans/0001-first-slice.md`;
`docs/plans/0002-what-to-work-on.md`; `docs/research/metis-deep-dive.md`;
`docs/research/beads-and-gastown.md §5`; `docs/research/SYNTHESIS.md §4.2-4.3`;
`.claude/skills/{decomposition,phase-transitions,beads,explore,writing-style}/SKILL.md`;
`.claude/skills/PROVENANCE.md`; `bd --help` output for `create`, `ready`, `update`, `dep`, `epic`
(bd 1.2.1, Homebrew).

The adopter (read-only, `the adopter's checkout`): `docs/notes/overnight-fleet-retrospective.md`;
`docs/notes/round-2026-08-15-evening-retrospective.md`; `docs/notes/round-2026-08-15-baseline.md`;
`docs/notes/round-2026-08-17-baseline.md`; `docs/notes/bead-admission-control.md`;
`docs/notes/bead-dedup-audit-2026-08-17.md`; `docs/notes/defer-sweep-2026-08-17.md`;
`docs/notes/task-specification-research.md`; `docs/notes/human-queue-triage.md`;
`docs/plans/0022-agent-working-procedure.md`.

Web (all accessed 2026-08-20):
- The 2020 Scrum Guide, https://scrumguides.org/scrum-guide.html
- The Kanban Guide v2025.5, https://kanbanguides.org/the-kanban-guide/
- Kanban University, The Official Guide to The Kanban Method, https://kanban.university/kanban-guide/
- Planview, "Why We Need WIP Limits", https://www.planview.com/resources/articles/wip-limits/ (secondary, for the Anderson phrase)
- Wake, "INVEST in Good Stories, and SMART Tasks" (2003), https://xp123.com/invest-in-good-stories-and-smart-tasks/
- Lawrence and Green, "The Humanizing Work Guide to Splitting User Stories", https://www.humanizingwork.com/the-humanizing-work-guide-to-splitting-user-stories/
- Patton, "Story Map Concepts" (PDF, 2013 Comakers), https://www.jpattonassociates.com/wp-content/uploads/2015/03/story_mapping.pdf
- Patton, "The New User Story Backlog is a Map" (2008), https://www.jpattonassociates.com/the-new-backlog/
- Adzic, "Forget the walking skeleton, put it on crutches" (2014; quotes Cockburn's definition), https://gojko.net/2014/06/09/forget-the-walking-skeleton-put-it-on-crutches/
- Kaltenecker and Leopold, "Flight Levels: A Short Introduction" (PDF), https://flowsphere.ch/wp-content/uploads/2024/02/Flight-Levels-A-Short-Introduction.pdf
