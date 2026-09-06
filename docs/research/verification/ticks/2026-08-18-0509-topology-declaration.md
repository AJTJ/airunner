# Tick 0509 — Named topology declaration: the minimal `.air/topology.toml`

**Date:** 2026-08-18. **Question:** what is the *smallest* declaration of a named topology such
that the machinery already specified for M0–M1 (ledger, hooks, hand-over gate, `air next`,
`air land`) runs the adopter's shape today and could run two or three other shapes later — without
Air growing a workflow engine.

**Sources read.** Local: `docs/decisions.md` (topology "flexible and *named*"; keep the coordinator;
"a *few* agents"; anti-cruft); `docs/research/SYNTHESIS.md` §4.1 (layers), §4.2 (step machine),
§4.3 (ten checks), §4.4 (presets); `docs/plans/0001-first-slice.md` §2–§6, §9 ("named topologies
(config comes once two shapes exist)"); `docs/plans/0002-what-to-work-on.md` §3, §5, §6;
`docs/research/prior-art-landscape.md` §A.2 (Gas Town roles/rigs), §A.4 (Symphony front matter),
§A.5 (Claude Code hooks/`--agents`), §B1.12 (ralph loop); `docs/research/beads-and-gastown.md`
§2.2–2.4; `docs/research/claude-code-billing.md` §7; ticks 0430 (measurement columns), 0445
(`air land`, batching trigger). Primary (all accessed 2026-08-18): openai/symphony `SPEC.md`
(§5.3/6.2/6.3/6.4/8.2/8.3/10.5) and `elixir/WORKFLOW.md` (front matter, "Backlog -> out of scope
for this workflow; do not modify"); gastownhall/gastown `docs/agent-provider-integration.md`
(`role_agents`, rig/town settings — `docs/concepts/rigs.md` returned 404, rig facts taken from
the landscape §A.2 fetch of `docs/design/architecture.md`/glossary); Anthropic
`code.claude.com/docs/en/sub-agents` (agent frontmatter fields, `--agents` JSON) and
`code.claude.com/docs/en/agent-sdk/subagents` (`AgentDefinition`, filesystem `.claude/agents/`).

## 0. Verdict

A topology in Air is **a table of roles plus three small tables (limits, gates, escalation)** —
about 25 lines of TOML. It names *who may cause which transition of the fixed step machine*, *how
many of each*, and *which of the existing checks are advisory or blocking for that role*. It
declares **no** routing, phases, prompts, scheduling policy or coordinator logic: those are either
Air's fixed machinery (step machine, `next` ranking, `land` steps) or a person's/skill's judgement.
The adopter's preset falls out of plan 0001 verbatim; `solo-ralph` and `pair-review` need no new
schema key; `refinery` needs one new *role kind* (a mechanical role with no model) and is gated by
the batching trigger from tick 0445. Do **not** write the file before the second shape exists;
until then the three knobs M0/M1 actually read live as flat keys in `.air/config.toml` (§6).

The prior art agrees on the boundary. Symphony's `WORKFLOW.md` front matter is only tracker,
polling, workspace, hooks, limits and the agent command (SPEC §5.3/6.4; the *behaviour* is prose in
the body of the same file — the agent reads it, the scheduler does not). Gas Town's only
per-topology declaration is `role_agents` (which CLI preset runs `witness`/`polecat`/`refinery`/
`crew`) plus `scheduler.max_polecats`; every other role behaviour is Go code + prompts. Claude Code
agent files declare `name/description/tools/model/effort/permissionMode/maxTurns/isolation/hooks`
and a prompt body — again: identity, budget, tools, model; never a workflow. So a "topology" file
that is *more* than roles + limits + gates would be more than any of the three ships.

## 1. Schema

```toml
# .air/topology.toml — committed, human-diffable; unknown keys are refused (anti-cruft)
schema = 1
name = "the adopter"        # a preset name (built into the binary) or a free name
extends = "the adopter"     # optional: start from a preset, override below

# ---- roles: who plays, where they run, what they may cause ----------------------
[roles.<role>]
where   = "main" | "worktree" | "any"        # how Air resolves the caller's role from cwd
                                             #   (git common dir == cwd → main; else worktree);
                                             #   env AIR_ROLE=<role> overrides for spawned sessions
backend = "claude-session" | "api" | "none"  # later: "codex" (Symphony codex.command), "acp"
model   = "opus" | "sonnet" | "haiku" | "<id>"   # required for api; advisory for sessions
effort  = "low" | "medium" | "high" | "xhigh" | "max"   # same
count   = 1  |  { max = 4 }  |  { min = 1, max = 4 }
wip     = 2                                  # live claims per member of this role (check 9)
may     = ["triage", "claim", "release", "reassign", "record", "handover",
           "review", "land", "close", "override"]
           # verbs = Air commands = edges of the fixed step machine (SYNTHESIS §4.2);
           # a verb not listed is refused for that role with a reason
run     = "air land --queue"                 # backend = "none" only: the command a tick runs

# ---- limits: counters Air already holds ---------------------------------------
[limits]
max_concurrent = { <role> = N, ... }         # Symphony agent.max_concurrent_agents, per role
by_state       = { awaiting_review = 6 }     # Symphony max_concurrent_agents_by_state; optional,
                                             #   plan 0002 §3: measure L first, cap later
build_slots    = 2                           # check 9 admission counter; optional
max_handover_attempts = 20                   # ralph-loop iteration cap; counts
                                             #   claims.handover_attempts (tick 0430)

# ---- gates: mode of an EXISTING check, per role --------------------------------
[gates.<check>]                              # checks are Air's, fixed: handover,
<role> = "off" | "advisory" | "blocking"     #   peer_overlap, recorded_green_at_land, review,
                                             #   citation, design_present, wip, merge
                                             # missing = the check's built-in default

# ---- escalation: labels withheld from `next`, and who is asked -----------------
[escalation]
labels = ["human", "owner"]                  # bd labels; `air next` withholds them
route  = ["coordinator", "owner"]            # order Air prints in the teaching denial

# ---- observe: names Air measures but never operates ---------------------------
[observe]
relay_tool = "SendMessage"                   # PostToolUse matcher for the relay metric (0430 §2.6)
```

Every key maps to a column, counter or check that plan 0001/0002 already specifies:

| Key | Consumed by | Existing fact it binds to |
|---|---|---|
| `roles.*.where`, `AIR_ROLE` | every `air` command and hook (`air whoami`) | worker identity = worktree name / `BEADS_ACTOR` (as-built §1.2) |
| `roles.*.may` | `air claim/handover/land/close/…` refuse with reason | step machine, forward-only table (SYNTHESIS §4.2; check 3) |
| `roles.*.wip`, `limits.*` | `air claim`, `air next`, `air status` | check 9; `claims` table; Symphony §8.2 eligibility ("global + per-state slots free") |
| `roles.*.backend/model/effort` | `air spawn` (M2) passes `--model/--effort`; `air status` shows drift for hand-started sessions | Claude Code agent frontmatter `model`/`effort` (fetched 2026-08-18); billing §7 mixed backends; cost ledger unit per backend |
| `gates.*` | `air hook Stop/PreToolUse`, `air handover`, `air land` | plan 0001 §4/§5 advisory-then-blocking; decisions "advisory-only for one round" |
| `escalation.*` | `air next` filter; denial text | plan 0002 §4 (`human`/`owner` withheld); Gas Town escalation route Deacon → Mayor → Overseer (borrowed *shape*, not beads) |
| `observe.relay_tool` | `PostToolUse` matcher | relay metric (tick 0430 §2.6) |

What "role" resolution costs: one `git rev-parse --git-common-dir` (≈10 ms, tick 0315) or an env
read; no `bd` call. What a role does *not* carry: a prompt. Prompts/skills belong to `CLAUDE.md`,
`.claude/agents/*.md`, `.claude/skills/` — the registers Claude Code already loads.

## 2. `the adopter` preset (from what the adopter actually does)

```toml
schema = 1
name = "the adopter"

[roles.coordinator]                 # the main-tree session (as-built §1.1; coord interview §5)
where   = "main"
backend = "claude-session"
count   = 1
may     = ["triage", "release", "reassign", "land", "close", "override"]
                                    # no "claim": anonymous claims from main were a named pain
                                    #   (as-built §4.4 "every session on the machine claims as AJTJ")
                                    # "land": land runs from the main checkout (`make land`, land.sh
                                    #   refuses non-main); today owner or coordinator presses it
[roles.worker]                      # backend-leaning, frontend-leaning, third-agent, fourth-agent
where   = "worktree"
backend = "claude-session"
count   = { max = 4 }               # SESSION_SOFT=4; corpus §0.6 returns thin past 3–4
wip     = 2                         # CLAUDE.md:524-538, counted in branches AND awaiting_review
may     = ["claim", "release", "record", "handover"]

[limits]
max_concurrent = { worker = 4 }
# by_state.awaiting_review: unset — measure L for a round first (plan 0002 §3, §7.5)

[gates]
handover              = { worker = "advisory" }        # M0; "blocking" after one round (decisions)
peer_overlap          = { worker = "advisory" }        # PreToolUse warn, never deny (plan 0001 §5)
merge                 = { worker = "off" }             # merges never refused (backend §7)
recorded_green_at_land = { coordinator = "advisory" }  # tick 0445: never block on a missing record

[escalation]
labels = ["human", "owner"]
route  = ["coordinator", "owner"]

[observe]
relay_tool = "SendMessage"          # the channel stays; Air measures it (coord interview §1)
```

Twenty-six lines. Everything a worker or coordinator experiences (what `next` shows, what
`handover` refuses, who may `land`) is derived from these plus the fixed machinery — no line here
says *how* to triage, *when* to merge, or *what* to say to whom.

## 3. Two more shapes in five lines each (proof the schema generalises)

**`solo-ralph`** — one worker, loop until evidence, no coordinator (landscape §B1.12; the
Anthropic ralph plugin's Stop hook blocks exit until done — Air's version blocks on *evidence*).
```toml
name = "solo-ralph"
[roles.worker]   where = "any";  backend = "claude-session"; count = 1; wip = 1
                 may = ["claim", "record", "handover", "land", "close"]
[limits]         max_concurrent = { worker = 1 };  max_handover_attempts = 20
[gates]          handover = { worker = "blocking" }     # Stop hook exit 2 until verify green at HEAD
[escalation]     labels = ["owner"];  route = ["owner"]
```

**`pair-review`** — worker + separate rubric-grader before `awaiting_review` (billing §7: review on
a metered API role; SYNTHESIS §4.4).
```toml
name = "pair-review";  extends = "the adopter"
[roles.reviewer] where = "any"; backend = "api"; model = "sonnet"; effort = "high"; count = 1
                 may = ["review"]                        # writes verify_runs(kind=review, exit)
[gates]          review = { worker = "blocking" }        # handover needs review exit 0 at HEAD
[limits]         by_state = { awaiting_review = 4 }      # only after L is measured
```
The grader's verdict is recorded as a `verify_runs` row (`kind = review`, exit code, log path) —
an evidence row, the same shape the gate already reads; the rubric itself is a skill file.

**`refinery`** (Gas Town shape, beads-and-gastown §2.4) — `extends = "the adopter"`, plus
`[roles.refinery] where = "main"; backend = "none"; count = 1; may = ["land", "close"];
run = "air land --queue"` and `coordinator.may` minus `land`. Needs the one new thing in the
schema (`backend = "none"`, a role that is a process, not a model) and is not to be built before
tick 0445's batching trigger fires (Σ(queue_depth−1)×land_duration ≥ 30 min/round and ≥ 8
landings/day with median depth ≥ 3).

## 4. What a topology does NOT declare (and why)

| Not declared | Where it lives instead | Reason from the record |
|---|---|---|
| Message routing, mailboxes, "who tells whom" | `SendMessage` (people/agents); Air only measures it | coord interview §1: the channel worked, the *facts* were wrong; corpus §0.4: keep exactly one inter-agent message; plan 0001 §6 "the binary never sends messages" |
| Coordinator logic (what to steer, priorities, rulings, re-cutting) | the coordinator's judgement + skills | decisions: keep the coordinator; SYNTHESIS §1b: "no LLM middle-manager"; plan 0002 §5 right-hand column |
| A phase machine per epic/feature | none; edges (`blocks`) + `bd ready` frontier | the adopter 0022: a phase machine "will be exactly that kind of wrong the first time real work does not fit its model"; plan 0002 §6 |
| Prompts, personas, skills, tool allow-lists | `CLAUDE.md`, `.claude/agents/*.md`, `.claude/skills/`, `settings.json` | Claude Code already owns these registers (sub-agents doc, fetched 2026-08-18); duplicating them is a second policy language (corpus §5) |
| Scheduling policy beyond counters (ordering, ranking, backoff, retries) | fixed in Air: `next` ranks by same-file overlap; Symphony order priority→oldest→id | plan 0001 §3 ranking is ours to *measure*, not configure; Symphony puts ordering in the SPEC, not the front matter |
| Step machine / transition table | fixed in Air (SYNTHESIS §4.2, forward-only) | metis pattern; a topology picks *who* may take an edge, never adds edges |
| Merge/land steps, refuse strings | `air land` (tick 0445) | behaviour-for-behaviour port; `land-prove` must stay green |
| Hook wiring | `air install` | plan 0001 §5; enforcement rank 10 (resolved-path check) |
| Time-based expiry, reclaim thresholds | ledger row semantics; M1 lease design | decisions: no time-based expiry; leases are generation-based |
| Which feature to work on | owner (`docs/decisions.md`) | plan 0002 §1 |

The one-line rule: **the fleet is a workflow, not a multi-agent system** (SYNTHESIS §1b) — the
declaration therefore names roles, counts and gates the way a `Makefile` names targets, and leaves
every conversation to the people and every step to the machinery.

## 5. Validation and location

**Where:** `<repo>/.air/topology.toml`, committed (same directory as `config.toml`; the ledger and
events are gitignored). Presets are compiled into the binary; `air topology init <preset>` writes
the preset out in full (no hidden defaults) so the diff is the whole truth.

**`air topology check`** — a probe, not a linter with opinions. It parses with unknown-keys-refused
and prints the *effective* table (preset + overrides) and the resolution for the current cwd
(`you are: worker/frontend-leaning; may: claim release record handover; wip 1/2`). Red conditions
(each ships with a red/green fixture under `probes/topology/`, run by `air selftest`):

1. unknown key or unknown verb/check/mode string (teaching denial names the valid set);
2. a role without `where` or `backend`; `backend = "api"` without `model`; `backend = "none"`
   without `run`; a `run` on a model backend;
3. `where` values that leave `main` or `worktree` unmatched (every caller must resolve to a role)
   or two roles with the same `where` (ambiguous resolution) — `AIR_ROLE` disambiguates only for
   spawned sessions;
4. no role may `land` (nothing can reach `landed`), or no role may `handover` while some role may
   `claim`;
5. `limits.max_concurrent[role]` > `roles.role.count.max`; `by_state` naming a state that is not
   in the step machine;
6. `escalation.labels` empty (then `next` would offer `owner` beads).

**Runtime posture** (Symphony SPEC §6.2/6.3, adapted): an invalid file at hook time never blocks —
hooks fail open with a one-line warning and behave as the `the adopter` preset; mutating CLI
commands (`claim`, `handover`, `land`) print the same warning and refuse only what the *built-in
default* would refuse. "Keep operating with the last known good effective configuration and emit an
operator-visible error" is exactly the behaviour; the last known good is the compiled preset.

## 6. When it becomes worth building

Trigger (plan 0001 §9): **the second real shape is about to run** — someone is going to run
`solo-ralph` on a side repo, or `pair-review` after a round of review-latency data says the human
reviewer is the constraint (SYNTHESIS §1b, corpus §0.5). Not a trigger: wanting the file for
tidiness, or Gas Town envy. Until then:

- M0/M1 hardcode the `the adopter` preset; the three knobs they actually read —
  `handover_gate = "advisory" | "blocking"`, `wip_per_worker = 2`, `max_workers = 4` — sit as flat
  keys in `.air/config.toml` next to `verify`, `shared_files`, `bd` path (plan 0001 §2), and migrate
  into `topology.toml` on the day the second preset lands (`air topology init the adopter` reads
  them);
- `air whoami` (role resolution by cwd) can ship in M0 for free because `air status` needs the
  same main-vs-worktree fact; nothing else of §1 exists as code before the trigger;
- the measurement that decides `pair-review`/`refinery` (L, queue depth, land duration) is already
  in the ledger (ticks 0430, 0445), so the trigger is observable, not a guess.

Cost of waiting: none — the schema above is data the machinery already keys on; adding the file
later is a parser plus six probes. Cost of building early: a config surface nobody diffs, and the
temptation to put routing/phases in it (the Gas Town failure mode, decisions "anti-cruft").

## 7. Sources (access dates)

- openai/symphony `SPEC.md` — https://raw.githubusercontent.com/openai/symphony/main/SPEC.md,
  2026-08-18: §5.3/6.4 front-matter keys and defaults (`agent.max_concurrent_agents` 10,
  `max_concurrent_agents_by_state` map, `codex.command` etc.); §8.2 eligibility ("Global concurrency
  slots are available. Per-state concurrency slots are available."); §8.3 per-state fallback to the
  global limit; §10.5 "A run MUST NOT stall indefinitely waiting for user input"; §6.2/6.3
  validation ("keep operating with the last known good effective configuration").
- openai/symphony `elixir/WORKFLOW.md` — https://raw.githubusercontent.com/openai/symphony/main/elixir/WORKFLOW.md,
  2026-08-18: front matter (`tracker`, `polling`, `workspace`, `hooks`, `agent`, `codex`); body is
  agent prose; "Backlog -> out of scope for this workflow; do not modify".
- gastownhall/gastown `docs/agent-provider-integration.md` —
  https://raw.githubusercontent.com/gastownhall/gastown/main/docs/agent-provider-integration.md,
  2026-08-18: `role_agents` per role (`witness`, `polecat`, `refinery`, `crew`), rig-level
  `settings/config.json` `agent`, town `default_agent`; resolution order; per-role model/args not
  configurable. `docs/concepts/rigs.md` → 404 (2026-08-18); rig = one repo under management,
  polecats/refinery are worktrees of `mayor/rig` — from `docs/research/beads-and-gastown.md`
  §2.2–2.4 (fetched 2026-08-17/18) and `prior-art-landscape.md` §A.2.
- Anthropic Claude Code subagents — https://code.claude.com/docs/en/sub-agents, 2026-08-18:
  frontmatter fields (`name`, `description`, `tools`, `disallowedTools`, `model`, `permissionMode`,
  `maxTurns`, `skills`, `mcpServers`, `hooks`, `memory`, `background`, `effort`, `isolation`,
  `color`, `initialPrompt`), locations (`.claude/agents/`, `~/.claude/agents/`, `--agents` JSON).
- Anthropic Agent SDK subagents — https://code.claude.com/docs/en/agent-sdk/subagents, 2026-08-18:
  `AgentDefinition` fields (`description`, `prompt`, `tools`, `model`, `effort`, `maxTurns`, …),
  `agents` option, filesystem definitions loaded from `.claude/agents/`, spawn caps
  (`CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`, `maxBudgetUsd`).
- Local: `docs/decisions.md`; `docs/research/SYNTHESIS.md` §1b, §4.1–4.4; `docs/plans/0001-first-slice.md`
  §2–§6, §9; `docs/plans/0002-what-to-work-on.md` §3–§6; `docs/research/prior-art-landscape.md`
  §A.2, §A.4, §A.5, §B1.12; `docs/research/beads-and-gastown.md` §2.2–2.4;
  `docs/research/claude-code-billing.md` §7; `private/research/adopter-as-built.md` §1.1–1.3, §4;
  `docs/research/coordinator-interview-2026-08-17.md` §1, §5; ticks 0315, 0430, 0445.
