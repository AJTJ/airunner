# 0002 — What to work on: the one procedure from feature to landed bead

Status: **decided and built in part, 2026-08-21; supersedes the 2026-08-18 draft.** Owner
decisions dated in [`../decisions.md`](../decisions.md) (2026-08-18, 2026-08-20, 2026-08-21).
Operational checklists stay in the skills: `decomposition` (cutting), `phase-transitions`
(when a thing may move), `beads` (`bd` quoting and traps). This file is the procedure that
strings them together; it does not repeat them. Research: `docs/research/metis-decomposition-and-agile.md`.

Marks: **[built]** exists in `crates/` today; **[roadmap]** is on plan 0005; **[prose]** is a
rule people follow. Roles (who does what) are in [`../rules/roles.md`](../rules/roles.md).

## 1. The shape

```
feature (owner, a paragraph)  →  epic (bd, --design is the spec)  →  beads (bd, one acceptance each)
capture (Air inbox)  →  triage (coordinator)  →  bead  →  claim  →  work  →  hand-over  →  land  →  close
```

- **Feature**: owner's, not tracked; a paragraph in `decisions.md` or a plan
  (`decisions.md` 2026-08-17). Metis's Vision, Flight Level 3
  (`research/metis-decomposition-and-agile.md §3`).
- **Epic**: `bd create --type=epic` whose `--design` is the spec. Metis's Initiative.
- **Bead**: one worker session, one reviewable diff, one runnable acceptance. Metis's Task.
- **Capture**: one line in Air's inbox, not a bead, never `ready`. Metis's backlog item.

What lives where:

| Holder | Holds | Why |
|---|---|---|
| **beads** (truth for ownership) | epics, beads, `--design`, acceptance, `assignee`, priority, `blocks` edges, status, `--estimate`, comments | bd's atomic CAS decides claim races; queues are beads fields only (`decisions.md` 2026-08-20) |
| **Air ledger** (truth for evidence) | captures/inbox, claims history (attempts, release reason), `verify_runs` at sha, sessions, edit journal, leases, landings, events | what git/bd cannot re-derive; no time-based expiry (plan 0001 §2) |
| **git** | branches, worktrees, `main`, merge ancestry | derived at query time, never copied |
| **people** | which feature; how to cut; priorities; rulings; whether a capture is a bead | judgement, kept as skills (§6) |

Epic phases (`discovery → design → decompose → triaged → active → closed`) are **derived,
never stored** (`decisions.md` 2026-08-18 item 4; `phase-transitions/SKILL.md` "Epic").

## 2. Intake: capture → triage → bead

**Workers capture, they never file.** `bd create` is hard-denied to workers at launch
(`crates/cli/src/cmd/launch.rs`; `decisions.md` 2026-08-20). Capture costs one line and the
worker keeps working or releases with a reason.

```bash
air capture "<one line: what, where (file:line), why it matters>"   # worker or coordinator  [built]
air capture --for owner "<decision only the owner can make>"          # owner decision queue   [built]
air inbox            # coordinator: open captures, oldest first         [built]
air inbox --owner    # the owner's queue; attention `owner-decision-waiting`  [built]
```

**The coordinator triages inline** (`decisions.md` 2026-08-18 item 2, reaffirmed 2026-08-20).
For each capture, one of three outcomes:

1. **It is a bead.** Validate, dedupe, group (which epic?), then file and link:
   ```bash
   echo "<why; cites file:line>
   ## Acceptance Criteria
   <one runnable command or test name>" | bd create "<verb-first title>" --type=task \
     --parent <epic> -p <1|2|3> --description=- --validate --estimate <minutes>     # [built: bd]
   bd dep add <later> <earlier>            # ordering, and any file shared with an open sibling
   air triage <capture-id> --bead <new-id> # links the capture; records time-to-triage  [built]
   ```
2. **It is not worth a bead.** `air triage <id> --drop "<why>"` **[built]**.
3. **It is the owner's.** `air capture --for owner` (or it arrived that way); the owner walks
   `air inbox --owner` with an agent. `bd human` does not exist in bd 1.2.x and is never used
   (`decisions.md` 2026-08-21 E; `rules/adopting-air.md §3`).

What refuses what:

- `bd create --validate` refuses a task/feature/bug whose description lacks an
  `## Acceptance Criteria` heading. This is bd's check, a heading grep; Air does not
  re-implement it (`decisions.md` 2026-08-20 "Acceptance is required by beads, not Air").
- Content of the acceptance is the coordinator's: one observable condition runnable inside
  the worktree by the agent, written so that satisfying it *is* the work
  (adopter `task-specification-research.md:526-545, 634-637`, via research §5).
- `--estimate <minutes>` is recorded and compared with actual (measurement spec §2.7); never a
  gate.
- Citations (`file:line`) must open at filing; a bead whose description and acceptance disagree
  is not filed, it is captured for the owner (research §5, "Neither will ask").

Air measures inbox depth and time-to-triage (spec §2.8); that number, not a rule, decides
whether triage ever leaves the coordinator.

## 3. Decomposition: epic framing, skeleton, splitting, sizing, exit criteria

Run the `decomposition` skill's procedure. The commands and the commitments, in order:

**3.1 Frame the epic.** `--design` with exactly the headings Why / Approach / Lanes / Out of
scope / Risks / Done when / Later (`decomposition/SKILL.md` "Frame the epic"). "Done when" is
one command that is red on `main` now and exits 0 when the epic is complete. It is required
before any child may be claimed (`decisions.md` 2026-08-18 item 1) and may be tightened, never
removed (Metis ADR-003:55, research §4). If the whole epic is one sentence of diff, file one
bead under an existing epic instead.

```bash
bd create "<epic title>" --type=epic -p 2 --description "<why, one paragraph>" --design-file - < design.md
```

**3.2 Walking skeleton first.** Child 1 is the thinnest end-to-end path that makes "Done when"
runnable, even if it returns the wrong answer (Cockburn via research §2). Unknown approach:
child 1 is `--type=spike` whose acceptance is a written finding, and the rest waits on it.

**3.3 Split the rest** with Lawrence's pattern list in order (workflow steps, operations,
rule/data variations, interface method, major effort, simple/complex, defer performance,
spike last); stop at the first pattern that yields 2 to 5 roughly equal children
(`decomposition/SKILL.md` "Split the rest"). Horizontal layer cuts only when each layer is its
own lane and the edge is recorded. File the skeleton plus **one wave**; the rest stays under
"Later" in `--design`. Re-cut when `bd ready --parent <epic> -n 0` has fewer open beads than
idle workers.

**3.4 Sizing.** The unit is one worker session ending in `air handover` passing (research §7).
Split when the acceptance needs "and", the description is a checklist, it touches two lanes,
or the worker would have to choose an approach. Not below one reviewable diff. Record
`--estimate`; the ledger calibrates the coordinator (spec §2.7).

**3.5 Edges.** Between children, never on the epic (an epic edge propagates to every child,
adopter `bead-dedup-audit-2026-08-17.md:99-100`). Shared file between siblings means a
`blocks` edge, cut by whoever has the whole tree in view. `bd dep cycles` must be empty.

**3.6 Exit criteria per derived state** (full table in `phase-transitions/SKILL.md` "Epic"):

| State holds when | Evidence |
|---|---|
| design | `--design` has Why (cited), Out of scope, Approach, Lanes, Risks, a red "Done when" |
| decompose → triaged | skeleton + one wave filed with `--validate`; edges recorded; `bd dep cycles` empty; `bd ready --parent <epic> -n 0` non-empty |
| active | first `air claim` on a child (ledger `claims` row) |
| closed | every child closed or superseded (`bd epic close-eligible`) **and** `air record verify -- <Done when>` exit 0 on `main`; then `bd close <epic> --reason=-` naming shas |

**3.7 Re-cut on evidence** (coordinator judgement; Air only reports): a spike closes, cut its
dependents now; a landed sibling already satisfies a child, close it with the evidence; a
capture says the approach is wrong, stop filing and tighten `--design` or close the epic with
a successor.

## 4. Dispatch: modes, queues, claim, run to completion

**Coordinator modes** (`rules/roles.md` "Coordinator"; owner 2026-08-21): **active** means
every online worker has a bead, assigned without asking, and an idle worker is a coordinator
failure; **idle** means feed no one. Escalate to the owner only through `air capture --for owner`.
There is no cap on work in flight (§8).

**Per-worker queues from live state.** Queues are beads fields only: `assignee`, priority,
`blocks` (`decisions.md` 2026-08-20). Reads, then writes:

```bash
air status [--attention]                           # sessions, claims, green sha, holdings, inbox, awaiting-review count  [built]
air holdings [--file X]                            # who has edits where, across worktrees                             [built]
air lease status                                   # exclusive resources (:8080, simulator, chrome, runtime)          [built]
bd ready --parent <epic> --unassigned -n 0 --json  # the frontier (never the capped default)
bd list --status in_progress --json                # what each worker holds
bd dep tree <epic> --json                          # what each landing unblocks
bd update <id> --assignee <worker>                 # the suggestion; the claim is the fact
```

Rules: one live claim per lane (pick the highest-priority ready bead whose files no
in-progress bead holds); priority encodes the wave (skeleton `-p 1`, dependents `-p 2`, rest
`-p 3`); shared file means an edge, not an assignment; fewer ready beads than idle workers
means run §3 first; a stuck worker's bead is released, never reassigned while claimed.
`air next` (overlap-ranked frontier with stale-citation flags) is **[roadmap]**; until then
the coordinator does the ranking from the reads above.

**Claim.** `air claim <id> [--files a,b]` is the only claim path: runs `bd update --claim`
(bd's CAS decides races) and writes the ledger row in the same step, loud on either failure
(`decisions.md` 2026-08-20 "wrapped, not watched") **[built]**. Raw `bd update --claim` is
denied to workers. Exclusive non-file resources: `air lease take|release|status|break <resource>`,
holder tied to the session so a dead holder is a fact (`decisions.md` 2026-08-21 A) **[built]**.
Give a bead back with `air release <id> --reason <landed|abandoned|reassigned|superseded|false-premise|owner-gated>`
**[built]**; `false-premise` is the sanctioned "this bead's premise is gone" exit and counts
toward discarded work (spec §2.3).

**Run to completion** (`rules/roles.md` "Worker"): claim → implement, committing small →
`git merge main` → `air record verify -- <cmd>` → `air handover` → `bd update <id> -s awaiting_review`
→ next bead. Ending a turn after the claim is a failure. Blocker: one `air capture`, then
release or take unrelated work. Hooks are quiet unless actionable and changed; the channel
delivers stuck / idle-with-claim / silent-with-claim / gone-with-claim / handover-not-green /
inbox-waiting / owner-decision-waiting / lease-held-by-dead-session to the coordinator
(`crates/cli/src/cmd/status.rs`; `air mcp`) **[built]**.

## 5. Hand-over, review, landing

**The one refusal.** `air handover` (also the Stop hook, advisory unless `AIR_ENFORCE=1`)
refuses `awaiting_review`/close unless all hold, and prints the failing check and the fixing
command (`crates/hooks/src/gate.rs`) **[built]**:

1. `verify-green-at-head`: a `verify_runs` row, exit 0, at HEAD, from a foreground
   `air record verify`. Evidence, never model text.
2. `main-merged`: `merge-base --is-ancestor main HEAD`.
3. `claim`: this bead is claimed by this actor in the ledger.
4. `digest-present`: a digest under `digest_dir` newer than the claim, when the repo configures
   one (`decisions.md` 2026-08-21 D).

Flakiness is named, not retried: runs at one sha that disagree print
`flaky-at-head: N green / M red`; gate semantics stay latest-wins; fix or quarantine the test
(`decisions.md` 2026-08-21 ad-jklh) **[built]**. `air record` also flags `suspicious`
(green under 2 s or no output), `command-changed`, `dirty-tree`, and refuses backgrounded
commands **[built]**. WIP commits and merges are never refused; no hook blocks on a question.

**Review and landing.** `awaiting_review` is not closed. Landing is piecemeal to `main`, no
per-epic integration branch (`decisions.md` 2026-08-18 item 6). **`land` stays the repo's
(`make land` in adopter) until `air land` ships [roadmap]**; when it does it is land.sh
behaviour-for-behaviour with a `landings` receipt (plan 0001 §4; plan 0005 §3). The
coordinator lands; workers never land or push (launcher deny list).

**Close.** `bd close <id> --reason=-` with the landed sha and the acceptance evidence, only
after the commit is on `main`. Epic close per §3.6. Air records `claims.released_at`
with reason `landed`.

## 6. What Air enforces vs what stays judgement

| Air enforces or records | Machinery | Mark | Stays judgement (skill, prose) |
|---|---|---|---|
| Workers cannot file beads or claim raw | launcher `--disallowedTools` (`bd create`, `bd update --claim`, `air land`, `git push`) | built | Whether a capture is a bead, a drop, or the owner's |
| Acceptance heading present at filing | `bd create --validate` (bd's check) | built | Writing an acceptance that *is* the work |
| Capture → bead link, time-to-triage | `air capture` / `inbox` / `triage` | built | Grouping, dedupe, which epic |
| Claim is atomic and recorded with reason history | `air claim` / `air release` wrapping `bd update --claim` | built | Which bead a worker gets; priorities; lanes |
| Exclusive resource held by one live session | `air lease`; attention `lease-held-by-dead-session` | built | Which resources count as exclusive |
| Hand-over: green at HEAD, main merged, claim, digest | `air handover`, Stop hook; `AIR_ENFORCE=1` | built (advisory) | Whether the diff is good; review itself |
| Verify integrity: foreground, duration, output, flaky, command change, dirty tree | `air record` | built | Verify completeness (the repo's fitness) |
| Role from the checkout; edit journal; peer warning on a held path | `air hook` | built | Announcing intent; splitting a shared file |
| Attention conditions pushed once, re-pushed only on change | `air mcp` channel; `air status --attention` | built | What to do about them |
| Owner decision queue | `air capture --for owner`; `air inbox --owner` | built | The decision |
| Estimate vs actual; inbox depth; review wait; awaiting-review count | ledger, `air status`; `air metrics --round` | built (facts) / roadmap (report) | Reading them; re-sizing |
| Overlap-ranked frontier, stale-citation flag at claim and post-merge | `air next`, `air post-merge` | roadmap | Re-cutting siblings; superseding |
| Land with receipt; close by evidence | `air land` | roadmap (repo's `make land` meanwhile) | When to land; batching |
| Non-empty `--design` before a child claim; epic close needs "Done when" green on main | `air claim` / `air status` epic derivation | roadmap (prose until then) | Framing the epic; vertical vs horizontal; spike first |

## 7. Metrics this procedure produces

Definitions live in `docs/research/verification/ticks/2026-08-18-0430-measurement-spec.md`,
the single list; a new metric is added there first (`decisions.md` 2026-08-20).

- §2.1 Review latency L per bead (hand-over → landed), split into reviewer wait and rewind cost.
- §2.2 Single-agent success, two tiers (first hand-over green; landed at first attempt).
- §2.3 Discarded work hours (abandoned, superseded, false-premise, rewound).
- §2.4 Imported-red incidents (merged a red peer tip).
- §2.5 Stale-suggestion rate for `air next` (when built).
- §2.6 Coordinator relay count (merge/overlap messages), or `unmeasured`.
- §2.7 Estimate accuracy: actual / `--estimate`, median and spread per round.
- §2.8 Inbox depth and time-to-triage: the number that decides whether triage leaves the coordinator.
- Shown, never raised on: `awaiting_review` count and per-bead review wait in `air status`.

## 8. Not built, or explicitly rejected

| Item | Decision |
|---|---|
| Per-worker or fleet-wide WIP cap | Rejected 2026-08-21 ("there is no cap; we set our goals and finish them"); reverses the `awaiting-review-over-cap` condition built that day and retires plan 0001 check 9. Measurement only. |
| Stored epic phase (label or field) | Tabled 2026-08-18 item 4; derive and display. |
| Metis hierarchy as a third tracker, fourth level, Ralph decompose loop | Rejected (research §4 "Reject" 1, 2, 5; adopter 0022:227-231). |
| Sprints, points, velocity, ceremonies | Rejected (research §4, §10); a stand-up is `air status`. |
| Auto-retry of a red verify | Rejected 2026-08-21 (ad-jklh): hides real reds; flakiness is named instead. N-of-M agreement is an open owner policy. |
| Headless or looped workers | Rejected 2026-08-20: a human is always in the loop; launchers start interactive sessions. |
| Lane forms / auto-declared lanes via PreToolUse | Subsumed 2026-08-21 (ad-uwdg): holdings derive from the edit journal; `air claim --files` is intent, measured not required. |
| Claims derived by watching `bd` commands | Rejected 2026-08-20 ("no flimsy watchers"); wrapped instead. |
| Separate triage session; auto-file `triage-needed` when captures outrun triage | Not now (2026-08-18 item 2, 2026-08-20); §2.8 is the trigger. |
| Per-epic integration branches | Not pursued 2026-08-18 item 6; revisit on a measured cross-sibling breakage. |
| Sibling-worktree fencing | Not pursued 2026-08-20; zero incidents. |
| Gate beads (`human`/timer/`gh:*`), convoys, formulas, molecules, merge queue | Not built (plan 0002 draft §6; Gas Town cautionary case, `decisions.md` 2026-08-18). |

### Decisions folded in (owner, dated)

- 2026-08-18: "Done when" required before children may be claimed; triage inline; `--validate`
  always on and workers capture, never file; phases derived not stored; WIP measured not
  enforced; integration branches not pursued.
- 2026-08-20: claims wrapped (`air claim`/`release`), raw `bd update --claim` and `bd create`
  denied to workers; coordinator builds queues in beads fields only; human always in the loop;
  channel push replaces the wake cron; machinery over Markdown; `--estimate` recorded and
  compared; acceptance required by bd's `--validate`.
- 2026-08-21: `air lease` built; digest is the fourth gate check; owner queue via
  `air capture --for owner`; verify integrity recorded; flaky named, no retry; no WIP cap;
  coordinator active/idle modes; `land` stays the repo's for round one.

## 9. Sources

Air: `docs/decisions.md` (2026-08-17 to 2026-08-21); `docs/plans/0001-first-slice.md` §2-§4;
`docs/plans/0004-first-round-surface.md`; `docs/plans/0005-roadmap.md`; `docs/rules/roles.md`;
`docs/rules/adopting-air.md`; `docs/research/metis-decomposition-and-agile.md` (§2 agile
sources with access dates, §4 steal/adapt/reject, §5 adopter evidence, §6-§9 procedure,
sizing, exit criteria, queues); `docs/research/verification/ticks/2026-08-18-0430-measurement-spec.md`;
`.claude/skills/{decomposition,phase-transitions,beads}/SKILL.md`; code:
`crates/hooks/src/gate.rs`, `crates/cli/src/cmd/{capture,claim,lease,record,status,launch}.rs`
(read 2026-08-21).

Metis (`~/projects/metis` at `6745810`) and adopter (`~/projects/adopter`
at `71191e0`) paths as cited in the research report §13. Primary agile texts (Scrum Guide 2020,
Kanban Guide v2025.5, Wake 2003, Lawrence and Green, Patton, Cockburn via Adzic, Kaltenecker and
Leopold), all accessed 2026-08-20, URLs in research §13. The 2026-08-18 draft's external
sources (Symphony, Gas Town, beads docs, spec-kit, Anthropic best practices) are in
`docs/research/verification/ticks/2026-08-18-0230-what-to-work-on.md` §5.
