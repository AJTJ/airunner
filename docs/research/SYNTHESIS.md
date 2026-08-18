# Synthesis — do we need a runtime, what shape, and what to build first

Status: draft v1, 2026-08-17. Derived from the eight reports in this directory; every claim below
points at the report (and through it to the primary source).

> **Verification status (2026-08-18).** Every sentence carrying a number, a named study/tool/person, or a causal claim was ledgered in
> [`verification/ticks/2026-08-18-0330-synthesis-claim-ledger.md`](verification/ticks/2026-08-18-0330-synthesis-claim-ledger.md):
> 94 claims — 60 verified (17 against a verification slice/tick, 37 against the `adopter-notes/` copies, 6 already corrected), 9 stale items fixed in this pass, 19 unverified (14 external/design facts from the landscape/billing reports, 5 adopter-internal numbers whose source file is not in the copies), 7 opinion. Stale items were fixed in place below with a citation to the verification row; the remaining unverified items are external
> facts no verification slice has re-fetched (landscape/billing) or adopter-internal numbers whose source file is not in `adopter-notes/`.

## 0. The answers, briefly

| Owner's question | Answer the evidence supports |
|---|---|
| Is a full "runtime" needed, or a thinner enforcement layer? | **Start as an enforcement layer; let it become a runtime by accretion.** by our tally its 66 process rules are only 27% enforced ([enforcement §2](adopter-enforcement-and-skills.md); adopter's own self-counts — six rules "still prose only", 8–9 of 14 protocol items `NOT ENFORCED` — agree in shape) and *nothing* fires on a sequence or a session event — its plan 0022 already specifies the missing hooks and none is written ([enforcement §0, §2](adopter-enforcement-and-skills.md)). The rank-1 and rank-2 pains (stalls; exit gated on a promise instead of evidence) need a process *outside* the session, which is where "runtime" starts ([enforcement §3](adopter-enforcement-and-skills.md)). |
| Runtime drives Claude Code, or Claude Code calls into the runtime? | **Both, in two layers, in this order.** Layer 1: Claude Code calls into it (hooks → `air hook …`; agents run `air claim/handover` the way they run `bd`). Zero change to how adopter starts sessions today. Layer 2: the runtime spawns/supervises sessions (`claude -p --worktree`, watchdog, reclaimer). Layer 2 is required for stall detection/escalation, which cannot be done from inside a stalled session ([as-built §4.2](adopter-as-built.md), [control-surfaces §2](claude-code-control-surfaces.md)). |
| Cost / how workers run | **Workers are Claude Code sessions on the subscription; API-backed roles only where cheap.** Headless/worktree/subagent sessions draw from the same 5-hour + weekly pool; ~3–5 concurrent workers is the practical Max ceiling — which matches adopter's fleet of 4 ([billing §1, §5](claude-code-billing.md)). Agent SDK may currently use subscription auth (separation paused 2026-06-15) but that is policy, not contract ([billing §3](claude-code-billing.md)). Roles are backed by a pluggable backend so a Haiku API call can do triage/digest while coding stays on sessions ([billing §7](claude-code-billing.md)). |
| Adopt something existing? | **No wholesale adoption exists**; nothing is a Rust, beads-native, policy-enforcing runtime. Adopt beads (via `bd --json`), Symphony's SPEC as the dispatcher vocabulary, `agent-client-protocol` as a worker protocol, and Vibe Kanban's Rust crates for executors/worktrees; build gates, leases, merge queue, human queue, cost ledger ourselves ([landscape §H, §J](prior-art-landscape.md)). Gas Town: copy the bones, don't run it ([beads-and-gastown §2.5–2.6](beads-and-gastown.md)). |
| Topology | Named, declared topologies are cheap once roles + gates are data: adopter's shape is one of several the same machinery runs (§4.4 below). |

## 1. What adopter taught us (evidence-backed)

**Working** ([as-built §3](adopter-as-built.md)): second round had zero fleet conflicts and zero idle (idle self-reported, `round-2026-08-15-evening-retrospective.md:30`); `make land` caught "green alone / red together" twice; verify went 238–284 s → 50 s; ~131 beads/day; friction beads fixed same day; dedup only ~3%. The gate that works is `make land`: refuse → one digest → `--no-ff` → verify on the merged tree → rewind on red → close attributable `awaiting_review` beads ([as-built §2.5](adopter-as-built.md)).

**Not working** ([as-built §4](adopter-as-built.md); [enforcement §3](adopter-enforcement-and-skills.md)):
- Direction: the ship path deferred and invisible for a month; 113/304 closed beads were fleet-on-fleet; no coordinator digest; coordinator relays were the least reliable channel.
- Liveness: a 4.5 h interactive-prompt stall; 51 min at WIP 0; reclaim threshold 8 h vs 4.5 h damage.
- Tracker lying: `bd close` on an already-closed bead echoes success and stores nothing (the earlier "`--claim` reopens closed beads" report was retracted by adopter — measured false, `main-agent-protocol.md:136-139`; `bd update -s in_progress` *does* silently reopen); `ready` caps at 100; `--notes` overwrites; anonymous claims from main; validation *warns*, teaching fabrication.
- Rules in the wrong register: `CLAUDE.md` 977 lines / ~16.6k tokens per session; rules written three times before they bound; "add a rule to CLAUDE.md" measured as a dead end.
- Enforcement inert or agent-editable where it lives: the Claude Code guard runs `main`'s copy so a worktree edit changes nothing, while the installed git hooks resolve to the *committing worktree's* copy so an agent can edit its own guard ([as-built §4.6](adopter-as-built.md); [enforcement W1](adopter-enforcement-and-skills.md)); guards that pass on nothing.
- 9 of 14 protocol items `NOT ENFORCED` per adopter's own retrospective.

**The one-line lesson** (adopter's words, [enforcement §0](adopter-enforcement-and-skills.md)): what is enforced is *negative and artifact-shaped* (never push; digest exists); what is prose is *positive and sequential* (claim before edit; verify before hand-over; triage before claimable). The positive sequence is exactly what a runtime can own.

**adopter never proposed a standalone runtime** ([as-built §5](adopter-as-built.md)); it calls itself "a single-machine design — nothing here travels" and notes "Rust pays for tooling when the binary has to run on a machine that does not have your toolchain" (`metis-comparison.md:283-289`). Plan 0022 wants "a working procedure that multiple agents can lock into, and doesn't need to be held in context" (`0022:9-13`) via PreCompact/Stop/PostToolUse hooks and a capture→triage commitment point — none built. That is the mandate.

## 1b. What adopter's research corpus adds (and where it corrects the draft)

From [adopter-research-corpus.md §0, §3, §4, §5](adopter-research-corpus.md) — each item cites the adopter note and, through it, the primary source:

- **The fleet is a workflow, not a multi-agent system.** The LLM multi-agent literature (MAST, MetaGPT, AutoGen, debate) does not describe worktree fleets; single-machine concurrency (leases, CAS, OCC, jobservers, level-triggered reconciliation) and the human-coordination canon do (§0.1). *Consequence:* design the runtime as a scheduler/supervisor over a work ledger, not as "agents talking".
- **Verification is the only observation point.** Agents break fail-stop and report success they did not achieve (51% on impossible tasks, Opus 4.1 addendum; an anti-hack sentence cuts it to 19% ≈ 2.7×, and only 55%→35% on Opus 4.5 — **not** "9×", [verification](verification/mas-literature-part2.md)); any protocol terminating on self-report is unsound; verify the merged result; even green is not proof (5–30% of "verified" patches wrong) (§0.2). *Consequence:* every gate tests evidence; acceptance criteria become reproduction tests; make verify cheap.
- **Rules in context are not mechanisms** — Ontario checklist null result, CAID soft-isolation below single agent (PaperBench arm, [verification row 6/8](verification/fleet-size-partition-cadence.md)), adopter's "eight of fourteen are wishes" (§0.3). Confirms §0/§1 above.
- **Partition by file to manufacture independence; never synchronise.** File overlap is a strong predictor of merge conflicts (Dias et al. 2020: OR 6.13 for changes to a common MVC *slice* — not strictly "same file", and slice-disjoint work still conflicts on config/spine files; 73,504 scenarios / 125 Rails+Django projects — population and variable resolved in [verification rows 9–10](verification/fleet-size-partition-cadence.md)); **do not** quote it as "6× vs branch lifetime" — the 1.04–1.09 duration ORs are on a standardised variable and not comparable as a ratio (same file, correction). Partition quality dominates existence (CAID); sub-file claiming (Claim Plane, single-author preprint) and lock-only registries (grite — a *simulation*, not LLM agents) measurably fail; keep exactly *one* inter-agent message (§0.4). *Consequence:* lane declaration at claim (check 6) is load-bearing; peer chat is not.
- **The single human reviewer is the constraint** (throughput = 2/L; review latency measured nowhere) (§0.5). *Consequence:* the human queue and review-latency measurement are core features, not extras.
- **Fleet size: a few, not many.** Kim et al. (budget-matched) show returns thin beyond 3–4 agents and go negative where a single agent already succeeds often (β<0) — confirmed. **Correction ([verification](verification/fleet-size-partition-cadence.md)):** Kim's topology overheads (Independent 58% / Centralized 285% / Hybrid 515%) describe *n redundant solvers of one task + aggregator*, not one-agent-per-bead fleets, so the "5–9× orchestrator overhead" claim does not follow from it; and AgentRadio's +29.8 pp requires its messaging channel (division of labour alone is +7.2 pp at ~6× cost), which weakens "route read-only beads to fan-out". What survives for us: adopter's coordinator (owner's description, 2026-08-17) tells workers **when to merge** and **assigns beads based on what peers hold**, plus steering. Both dispatch duties are computable from state the runtime holds; the fragile part is the *relay* (least reliable channel per the retros). Recommendation: **keep the coordinator**; move computable dispatch into Air (hook-time injection, `air next`, hand-over gate) so the coordinator steers, sets priorities, and overrides. `adopter` (coordinator + workers) is the default preset; a coordinator-less preset is something named topologies let us *measure*, not a recommendation.
- **Resources:** supervisor owns the count; reclamation state lives in the kernel (`flock`, `SEM_UNDO`) or supervisor, never the holder; level-triggered reconciliation; deadlines ask, don't kill; cheap liveness signal beats smart detectors (§0.7). *Consequence:* leases live in the runtime's store with generations; the reclaimer converges regardless of quiescence.
- **Checkpoint = write the deliverable incrementally; crash-only; a wrap-up hook protects the wrong case** (§0.8). *Consequence:* check 7 journals *state* at zero token cost rather than asking the model to summarise on exit.
- **Task specs:** acceptance = one observable condition checkable inside the worktree; write the check, not the prose (§0.9). *Consequence:* triage (check 4) requires an executable acceptance, not a sentence.
- **CLI-first for agent-facing operations.** The corpus's "5–28× cheaper" figure is misattributed (it compares no-MCP scaffoldings to MCP-capable ones; paired ratios span 0.43×–29×) — what survives is equal failure frequency and 12.9% vs 2.2% wasted spend ([verification](verification/mas-literature-part2.md)); together with the interviews' preference for `bd`-like commands, that still supports CLI-first with MCP optional later.
- **Language (§5):** the corpus's measured case *against* Rust applies to repo-local gates behind `make` (0.3 s gap, 21 s cold build per worktree, guard blocks `cargo run`, regex-shaped work); it explicitly reopens for a **standalone binary that ships, owns long-lived state, speaks kernel primitives, and runs where the target repo's toolchain may not** — Metis's case (§5.4). Constraints it still imposes: policy in a human-diffable config (not a second policy language); gates as files with red/green probes; **never make the target repo's tooling depend on the runtime's build**; every enforcement ships with a paired probe and a teaching denial ("Code fails confidently").
- **Contradictions to respect (§4.1):** plan 0010 §8 *rejected* a blocking Stop hook while merge-automation proposes an advisory one and Metis's is the model — resolution: evidence-gated, never promise-gated; CAID vs STORM: [verification](verification/fleet-size-partition-cadence.md) finds worktree ≈ soft isolation in *both*; what moved between them was the single-agent baseline (53.1→66.4 with a model step) — Kim's β<0 regime. Worktrees are justified by conflict-avoidance and process safety, not by a benchmark win; the runtime should measure discarded-hours, not conflicts; `make lease-break` is both the priority-inversion fix and the split-brain path — design together (lease generations).
- **Open questions the runtime should be built to *measure* (§4.2):** single-agent success rate per bead; review latency; rule compliance vs rules-file length; whether `bd heartbeat` fires while blocked on stdin; four simultaneous `make verify` never timed.

## 2. What the world has (use / borrow / build)

From [prior-art-landscape §H/§J](prior-art-landscape.md) and [beads-and-gastown §5](beads-and-gastown.md):

| Need | Use | Borrow design | Build |
|---|---|---|---|
| Task/issue layer | **beads** via `bd --json` — rely only on the surface present in the tested 1.2.2 line: `ready`/`show`/`list`/`update --claim`/`update -s -a`/`comment`/`close`/`dep`/`blocked` ([tick 0300 §3c](verification/ticks/2026-08-18-0300-bd-1-2-x-facts.md)); CAS `--if-*`, `unclaim`, events journal, leases are 1.2.1/`main`-only, and formulas/molecules/gates are not needed ([tick 0230](verification/ticks/2026-08-18-0230-what-to-work-on.md)) | beads' lease schema (`lease_expires_at`, `heartbeat_at`, `granted_node`, grace = 2×TTL — on `main`, not in a tested release) | `WorkLedger` trait; feature-gate 1.2.x-only commands by version |
| Dispatcher / tick loop | — | **Symphony SPEC.md**: `WORKFLOW.md` front matter; tick = reconcile → validate → fetch → sort → dispatch ≤ limits; Unclaimed/Claimed/Running/RetryQueued/Released; run-attempt states; stall/turn timeouts; capped backoff; `/api/v1/state` | the loop |
| Worker protocol | `agent-client-protocol` 2.0; native Claude adapter (`claude -p --output-format stream-json`, `--worktree`, hooks exit-2 blocking, `total_cost_usd`); Codex `app-server` | Vibe Kanban `executors` | adapters |
| Worktrees | `git worktree`; Claude Code `--worktree` semantics | Vibe Kanban `worktree-manager`; Factory cleanup policy (remove clean, keep dirty, never delete branch) | policy |
| Merge / landing | adopter's `make land` (already the best local gate we have) | Gas Town Refinery (batch + bisect + gates); Overstory 4-tier conflict ladder + merge lock; "CI is the only gate" | our merge queue |
| Human decisions | ACP `session/request_permission`; beads `human` gates; adopter `human`/`owner` labels | Gas Town escalation severities + ack; "Needs You" queue | the queue |
| Watchdog / leases | — | Gas Town Witness/Deacon + GUPP ("no progress = violation"); Symphony stall/turn timeouts | leases in SQLite that survive restart |
| Cost | Claude `total_cost_usd`/`modelUsage`; OTLP | Paperclip budget hard-stops; Overstory per-bead cost | ledger per bead/run/model |
| Persistence pattern | — | **metis**: filesystem is truth, SQLite a disposable index; forward-only transition tables; short codes; read-before-edit mtime guard ([metis §4](metis-deep-dive.md)) | |
| Loop pattern | Anthropic ralph-loop plugin state file | metis Stop-hook contract (block exit until evidence) — but gate on `make verify` exit 0, **never on a token the model types** ([metis §3](metis-deep-dive.md); [enforcement §3 rank 2](adopter-enforcement-and-skills.md)) | |

Do not adopt: Gas Town wholesale ("chaotic and sloppy", ~$100/h, verification chain open); Restate/Temporal/Windmill (external server, wrong granularity); `beads_rust`/`br` (pre-Dolt, not interoperable); AGPL tools for code reuse.

## 3. Constraints that shape the design

- Rust; single machine first ("nothing here travels" is fine for v0; leases/paths must not assume it forever).
- Beads stays; we never write its store except through `bd`; we add a **commitment point** so `bd create` no longer equals `bd ready` ([enforcement §4.6](adopter-enforcement-and-skills.md)).
- Enforcement must live **outside the agent's reach**: installed hooks resolve to the runtime binary, and the runtime verifies the *resolved path*, not presence ([enforcement §3 rank 10](adopter-enforcement-and-skills.md)).
- Gates test evidence (exit codes, files, `bd` state), never model-typed tokens.
- Everything the runtime observes goes into an append-only event log; every gate decision is explainable ("refused because …") — adopter's guards that fail confidently are the anti-pattern ([as-built §4.6](adopter-as-built.md)).
- Cost is a first-class ledger column from day one (subscription sessions counted in turns/duration; API calls in USD).

## 4. Proposed shape

Binary name: **Air** (`air`), owner's choice 2026-08-17 ("for now").

### 4.1 Layers
1. **`air hook`** — one binary invoked from Claude Code hooks (`SessionStart`, `PreToolUse`, `PostToolUse`, `PreCompact`, `Stop`, `SubagentStop`). Reads hook JSON, consults the ledger, returns allow/deny/additionalContext. This is where the sequence rules become checks.
2. **`air` CLI for agents** — `air claim <bead>`, `air lane declare`, `air handover`, `air ask-human`, `air status`, `air next`; thin, JSON-out, mirrors `bd` ergonomics. CLI-first (the corpus's "5–28× cheaper" is a scaffolding effect, not an interface effect — what survives is equal failure frequency and 12.9% vs 2.2% wasted spend, [MAS-part2 row 42](verification/mas-literature-part2.md)); MCP optional later.
3. **`air daemon`** (layer 2) — dispatcher tick per Symphony; spawns/attaches sessions; watchdog (transcript mtime, turn/stall timeouts, GUPP); reclaimer; merge queue; human queue; cost ledger; `/api/v1/state`.
4. **`air land`** — port of `make land` semantics as a library + CLI (refuse → digest → `--no-ff` → verify merged tree → rewind on red → close by evidence), then the batch/bisect queue.

### 4.2 The enforced step machine (per bead)
`triaged → claimable → claimed(actor, lane, lease) → working(worktree, checkpoints) → verified(evidence: verify exit 0 at HEAD) → awaiting_review → landed → closed`, with `blocked`/`stale`/`escalated(human|owner)` side states. Transitions are a forward-only table (metis pattern); illegal transitions are refused with a reason; state is stored in beads (labels/status/metadata via `bd`) plus the runtime's SQLite for leases/evidence/cost.

### 4.3 The first ten checks (adopter's ranked list, [enforcement §3](adopter-enforcement-and-skills.md))
1. Stall detection + escalation + sane reclaim (Stop/idle hook + watchdog).
2. Exit gated on evidence: `verify` exit 0 at HEAD or a filed blocker bead (Stop hook).
3. Claim/close as a real state machine with actor; refuse illegal transitions; CAS.
4. Triage commitment point: a bead is not `ready` until acceptance + labels + edges exist.
5. Citation check before claim (`file:line` opens and matches, or acknowledge).
6. Lane declaration at claim; refuse overlapping second claim.
7. Machinery-written checkpoints (PostToolUse journal: bead, lane, touched files).
8. Non-empty `--design` before feature/epic claim.
9. WIP ≤ 2 and build-slot admission counters.
10. Hook installation currency + resolved-path check.

### 4.4 Named topologies
A topology is data: roles (name, backend, allowed transitions, tools), gates between steps, escalation targets, concurrency limits per role/state (Symphony `max_concurrent_agents_by_state`). Examples to ship as named presets:
- **`adopter`** (default) — coordinator session on main (steers, triages, sets priorities/lanes, overrides, runs `land`) + N workers in worktrees. Dispatch state (merge-now advisories, overlap-ranked candidates) is computed by the runtime and delivered at hook time, not relayed.
- **`solo-ralph`** — one worker, loop until evidence, no coordinator.
- **`refinery`** — workers + a merge-queue role that batches/bisects (Gas Town shape).
- **`pair-review`** — worker + separate rubric-grader role before `awaiting_review`.
Backends per role: `claude-code-session` (subscription), `api` (provider/model/effort, metered USD), later `codex`/ACP.

## 5. First productive slice (adopter can run each)

- **M0 (days):** `air hook` + `air` CLI, installed into adopter's `~/.claude/settings.json` alongside `cmd-guard.py`: checks 2, 3, 7, 10 above; ledger in `.air/ledger.db`; every decision logged. Measure: protocol items enforced goes from 5/14 to ≥9/14 in `make fitness`.
- **M1 (a week or two):** checks 1, 4, 5, 6, 8, 9; `air land` replacing `scripts/land.sh` behaviour-for-behaviour with `land-prove` still green; watchdog + reclaimer as a foreground `air watch` first (no daemon yet).
- **M2:** `air daemon` dispatcher (Symphony tick) that can *also* spawn `claude -p --worktree` sessions; human queue TUI; cost ledger; named topologies as config; second backend (`api`).

Non-goals for M0–M2: multi-host, web UI, replacing beads, running Gas Town.

## 6. Risks and open items

- **`bd` version trap** ([beads-and-gastown §0, §1.7](beads-and-gastown.md)): adopter runs 1.2.1 (accidental, untested); 1.2.2 (= the tested 1.1.2 code) drops leases/heartbeat/reclaim/events/`sync`/serve **and also** `unclaim`, the CAS guards `--if-*` (exit 13), `update --force` and `--brief`, and refuses the v65 schema without a cursor rollback ([tick 0300 rows 1.2, 2.3, 5.4](verification/ticks/2026-08-18-0300-bd-1-2-x-facts.md)). Decided (tick 0300 §3; plan 0001 §7): pin 1.2.2 (`brew pin`) after the rollback and let the ledger own CAS/leases; `bd` status/assignee are mirrors of the ledger, never inferred. The landscape report independently recommends the mirror.
- Hooks in `~/.claude/settings.json` are user-scoped; per-worktree settings must be written by the runtime, and the resolved path checked.
- Billing numbers move; re-verify [claude-code-billing.md](claude-code-billing.md) before any capacity decision (it is primary-sourced as of 2026-08-17).
- Measure, don't assume: single-agent success rate per bead, review latency, discarded hours with/without lane partition (corpus §4.2) — the runtime's ledger should make these free.

## 7. Sources
The eight reports in this directory, each with its own Sources section: `adopter-as-built.md`, `adopter-enforcement-and-skills.md`, `adopter-research-corpus.md` (+ `adopter-corpus-digests/`), `metis-deep-dive.md`, `prior-art-landscape.md`, `beads-and-gastown.md`, `claude-code-control-surfaces.md`, `claude-code-billing.md`.
