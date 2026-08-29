# Harness and orchestrator landscape

**Purpose**: a standing answer to one question, refreshed on a cadence rather than written once.
*Is Air a lesser version of something that already exists, and which parts of Air should be deleted
because a harness or a mature project now does them better?* The goal is the owner's productivity,
not Air's completeness, so every entry here is read for what it makes deletable.

**Relation to [`prior-art-landscape.md`](prior-art-landscape.md)**: that survey (2026-08-17) asked
"what should we use or borrow *before* building Air", was Rust-first, and predates the harness
question entirely (one mention of OpenClaw in 101 KB). This document asks "what should we *stop*
maintaining now that Air exists", covers the harness layer, and is designed to be re-run. Where the
older doc already went deep (beads, Gas Town, Codex CLI, Symphony, goose, Vibe Kanban) this one
points at it instead of repeating it.

**Method**: web survey 2026-08-24. Every one of the 186 rostered projects was read: repository
metadata (stars, language, license, creation date, last push, contributor count) came from the
GitHub REST API on that date, and each project's own README was fetched and read the same day.
Notes are therefore each project's claims about itself, checked for what it says rather than for
whether it works. §3 holds the eleven read in depth against Air's axes; Appendix A holds all of
them. Statistics in §2.5 are computed over the 181 repositories whose metadata resolved. Any number
quoted from a blog rather than a paper is labelled as such, after the correction in §5.4.

**Next refresh due**: 2026-09-24, or immediately after any Claude Code release that adds an
orchestration surface. Procedure in §6.

---

## 1. The axes

Nine axes, chosen because they are the things Air actually claims. A project that scores on the
first six is a candidate to replace part of Air; a project that scores on none of them is a session
manager and is out of scope no matter how popular.

1. **Durable work store**: is there a task record that survives the session, with dependencies and
   status, or is the work list a prompt?
2. **Atomic claim**: can two agents both take the same item? (Air: `air claim` over `bd`, CAS in the
   ledger.)
3. **Recorded evidence**: is a verification run persisted as a fact (command, exit status, commit),
   or only printed into a transcript?
4. **Evidence-gated completion**: can an agent mark work done *without* recorded evidence? If the
   answer is "yes, but it is told not to", the project does not have a gate.
5. **Isolation**: worktree, container, VM, or none.
6. **Human posture**: human *in* the loop (approves actions), *on* the loop (maintains the machine,
   watches), or *out* (wakes to a result). See §5.3, this axis is where Air is most exposed.
7. **Harness-agnostic**: does it work with any CLI agent, or is it welded to one?
8. **Cost to keep**: language, runtime, dependencies, how much of it the owner would maintain.
9. **License**.

---

## 2. Verdict, 2026-08-24

### 2.1 Commodity, and therefore deletable from Air on sight

Worktree per agent, tmux fleets, session dashboards, kanban boards over agent tasks, per-agent
notifications, "which session needs input" inboxes. The curated roster in Appendix A has 186 active
projects and roughly 70 of them are exactly this. Claude Code itself now ships worktree isolation,
subagents, a workflow runner, hooks, scheduled runs and background tasks as first-party surfaces
(observed directly in this session's tool surface, 2026-08-24; see also
[`claude-code-control-surfaces.md`](claude-code-control-surfaces.md)). Anything in Air whose job is
*spawn, isolate, watch* is being commoditised from below by the harness and from the side by 70
competitors, and none of it is where Air's leverage is.

### 2.2 Contested: Air's "distinctive" core, already built elsewhere

This is the finding that matters, and it is not the comfortable one. After reading all 186 rostered
projects (Appendix A), the count of projects that independently built some part of what this repo
treats as its own is **eleven**, not the two or three a shallow pass suggested.

| Air capability | Already exists as | Shape |
|---|---|---|
| Recorded evidence, gate on completion | **loki-mode** | `.loki/proofs/<run_id>/` receipts splitting machine facts (command, exit code, gate verdict) from AI judgment; `VERIFIED` requires a real command exiting 0 with a non-empty diff and nothing suppressed; diff hash recomputable by a stranger |
| Claim, lease, heartbeat, verify, gate, run ledger, in Rust | **tutti** | `tutti.toml` declares roles and workflows; loop is intake → run → review → gate → record; `tt issue-claim acquire\|heartbeat\|release\|sweep` |
| Coordinator plus worker-per-bead over `bd`, worktree each | **orc** | Runs `bd init`, decomposes a goal into beads, one engineer per bead per worktree, ephemeral reviewer, review before merge |
| Ledger plus a hook that refuses a write | **Zaivern Code** | Line-range leases in a per-repo lease ledger; git hooks refuse a colliding write; publishes the measurement (96 edits, 0 refused, 30 shifted) |
| Evidence-gated run outcome | **MartinLoop** | `VERIFIED` / `STOPPED` / `NEEDS REVIEW`; receipts carry verifier evidence, budget posture, integrity state |
| Verification signals plus tamper-evident history | **bernstein** | Janitor checks tests/lint/typecheck; HMAC-chained audit log with Merkle seal; receipts verified against a public key |
| SQLite work store, atomic claims, handoff notes over MCP | **guild** | Quests with atomic claims and dependencies, lore, briefs, one Go binary |
| Leases, heartbeats, retries, attempt-per-worktree | **Factory** | SQLite state, scheduler with run admission, attempt/result/failure recorded together |
| Work item plus audit trail plus "lands in review, not main" | **multica** | 47k stars, 23 agent CLIs, self-hosted; intent, run, decisions and diff stay connected |
| Scored completion verdict | **IM.codes** | `PASS` / `REWORK` / `BLOCKED` decides whether a run may pass or must repair |
| Maker/verifier handoff as a field on the work item | **5dive** | SQLite queue where every row carries assignee, verifier, and handoff position; bash plus systemd, runs on 1 GB |

Plus the ones that record without gating (**aGiTrack**, per-turn commits with model and token cost;
**codecast**, session triage inbox with line-level attribution) and the ones that gate without
evidence (**ORCH**, **Crewplane**, **kandev**, **Fletch**, **ivy-tendril**: review is a human or an
agent, not a recorded fact).

So "the ledger and the gate" is not unclaimed territory. It is thinly and repeatedly claimed, mostly
by projects younger than six months. A targeted deep-research pass on 2026-08-25 (108 agents, 25
sources, 124 claims extracted, 25 adversarially verified, 9 killed) added two more that the roster
does not carry, **AgentOps** and **agentic-os** (§3.10b), and confirmed that the specific
combination in §2.3 survives. State that survival as **"unclaimed among what was reached"**: it is
absence-of-evidence over a bounded set, and three sharper phrasings of the same negative were voted
down during verification for over-reach.

### 2.3 What is genuinely still Air's

The residue after §2.2 is narrower than the first pass claimed, and it is three properties rather
than a component:

- **The refusal is on the tracked work item, not on a run the tool itself orchestrated.** loki-mode,
  MartinLoop and bernstein all gate a run inside their own loop. Air refuses a `bd close` of a bead,
  whatever produced it, because `is_handover_command` matches the close and `AIR_ENFORCE=1` makes the
  refusal real. The unit of proof is the thing that survives across sessions, agents and tools.
- **The green must sit at a HEAD that already contains main.** No project found in the sweep ties
  evidence to a commit that has merged the integration branch. "Tests passed" and "tests passed on a
  tree containing everyone else's work" are different claims and only the second is worth anything
  in a fleet.
- **It holds in an interactive session a human is watching.** bernstein removes the model from the
  coordination loop, MartinLoop caps and rolls back, loki-mode runs spec-to-receipt: all aimed at
  unattended work. Nothing found gates an interactive, human-supervised session.

Everything else Air does now has a better-resourced twin. That is one hook, one SQLite table and one
convention, which is the same conclusion `do-less` reaches from the other direction.

### 2.4 Would the ledger and the gate port to another system?

Asked directly by the owner, 2026-08-24. Answer: the gate ports; the fleet does not need to come
with it.

The gate needs exactly four things: a way to observe the completion command before it runs
(Claude Code's PreToolUse hook, observed; OpenCode's `tool.execute.before` plugin hook,
<https://opencode.ai/docs/plugins/>, accessed 2026-08-24; Codex not checked), a place to write and
read verify runs (SQLite in the repo), the commit sha and its merge base, and the identity of the
work item. It does *not* need worktrees, it does not need to launch anything, and it
does not need a coordinator. Air already puts the gate in a hook rather than in the launcher, so the
launcher and the roles prose can go without touching it. The parts that would be painful to port are
the parts §2.1 says to delete anyway.

The real dependency is not the worktree, it is **the durable work item**. The gate needs something
with an id to refuse to close. Today that is `bd`. On another harness it would still be `bd`, or
guild's quests, or GitHub issues. Keep the gate keyed to an id and a sha, and it survives every
harness change; key it to Air's own session model and it does not.

---

### 2.5 Adopt one of them, or keep what works?

The owner's framing, 2026-08-24: Air works, it costs little to maintain, so switching needs a
positive case rather than a neutral one. Here is the case, with numbers rather than impressions.

**What the field looks like, measured.** Of the 181 rostered projects with resolvable metadata on
2026-08-24: median age **187 days**; **47%** created within the last six months and **90%** within
the last year; **15%** already have no push in over 90 days and 2% are archived; median contributor
count 11, but **29% have three or fewer** contributors. Only **26%** clear a minimal health bar of
ten or more contributors, a push within the last week, and at least six months of history. 90 of 181
are TypeScript, 20 are Rust. Licenses are mostly permissive (95 MIT, 43 Apache-2.0) but 21 declare
nothing GitHub recognises and 11 are AGPL-3.0, which matters if anything is ever distributed.

**What that means for adoption.** A dependency's expected life is the thing being bought, and in
this field the median dependency is six months old. The projects that overlap Air's core are worse
than the median on exactly the axis that matters: tutti has 3 contributors and 27 days without a
push; orc has 1 contributor and 64 days; Zaivern Code has 2 contributors and 35 days of history;
MartinLoop has 4; 5dive has 3; Crewplane has 3; loki-mode has 6 and is BUSL-1.1 source-available
rather than open source. The two projects with real teams behind them, multica (272 contributors)
and paperclip (194), are board-and-audit products that do not gate on evidence at all, so adopting
either would mean keeping Air's hook anyway.

**The switching cost is not the install.** It is the procedure. Air's value in this repo is not its
code, it is that `bd close` refuses without a recorded green, that workers close their own beads with
proof, and that the owner has a written record of why each mechanism exists. Adopting an orchestrator
replaces the launcher and the board, which are the cheap parts, and leaves the procedure to be
re-encoded in the new tool's vocabulary. Every project in Appendix A that got close to Air's
procedure invented its own nouns for it.

**Objectively, three positions are defensible.** They are not equally good.

1. **Keep Air, delete its commodity half, keep the gate.** Cost: the deletions in §2.1, and a monthly
   hour on this document. Risk: continuing to maintain a launcher and a coordinator that the harness
   is absorbing. This is the recommendation, because the thing Air is uniquely good at is also the
   thing that is cheapest to keep.
2. **Adopt a commodity manager for the fleet, keep Air's hook and ledger.** The candidate is scion
   (Google-published, 26 contributors, container per agent, harness-agnostic, tmux attach/detach) or
   herdr for a pure terminal fleet. Air keeps the hook, the ledger and `bd`. Cost: one integration,
   plus whatever the manager assumes about how work is dispatched. This becomes correct the moment
   the fleet grows past what one person can watch, or the isolation needs to be stronger than a
   worktree.
3. **Switch wholesale to tutti or loki-mode.** Not recommended today, on the numbers above: both are
   younger and thinner-staffed than the thing they would replace, and loki-mode's license is not one
   to build a workflow on. Revisit if either grows a team, or if Air's gate turns out to need
   features one of them already has.

**The honest asymmetry**: adopting is cheap to try and expensive to reverse once the procedure has
been rewritten in someone else's nouns; not adopting costs an hour a month of watching. Given that
Air currently works, watching is the better trade until one of the neighbours passes the health bar.

## 3. Verified entries

Read from primary sources on 2026-08-24 unless noted. Ordered by relevance to the axes in §1.

### 3.1 bernstein
Python 3.12+, Apache-2.0. <https://github.com/sipyourdrink-ltd/bernstein> (accessed 2026-08-24).
Decomposes a goal into tasks, spawns agents in separate git worktrees across 40+ CLI agents,
verifies, merges to main. No LLM in the coordination loop: scheduling is plain Python, so replaying
yesterday's plan reproduces yesterday's task graph. Verification is by concrete signal (tests pass,
files exist, lint clean, typecheck clean) checked by a "janitor" after the agent finishes. Keeps an
always-on lineage spine, an opt-in HMAC-chained audit log (`BERNSTEIN_AUDIT=1`), a replay journal,
and signed run receipts a reviewer can verify without access to the live system. Solo-maintained,
beta.
*Overlap with Air*: high on axes 3, 4, 5. Human posture is "out". The closest thing found to Air's
gate, arrived at from the unattended direction.

### 3.2 MartinLoop
TypeScript / Node 20+, Apache-2.0. <https://github.com/Keesan12/martin-loop> (accessed 2026-08-24).
Execution control for coding agents: `--budget-usd` and `--max-iterations` hard caps, allow/deny
path policy checked before execution, blocked unsafe verifier commands, rollback-aware audit trail.
Runs terminate as `VERIFIED`, `STOPPED` or `NEEDS REVIEW`, where the distinction is whether evidence
supports completion. `martin share --latest` emits JSON, Markdown and SVG receipts.
*Overlap with Air*: this is the evidence-gated completion idea, named and shipped. Difference is the
unit (a run, not a tracked work item) and the posture (unattended).

### 3.3 guild
Go 1.25+, Apache-2.0. <https://github.com/mathomhaus/guild> (accessed 2026-08-24).
One compiled binary containing an MCP server over embedded SQLite in `~/.guild/`. Stores "quests"
(tasks with priority, dependencies, file references, **atomic claims preventing simultaneous
ownership**, cascade unlocking of dependents), "lore" (typed knowledge with auto-staling), and
"oaths and briefs" (principles auto-loaded at session start, handoff notes between sessions). Hybrid
BM25 plus vector search. Local only, no API keys.
*Overlap with Air*: this is Air's ledger, claims and handoff surface, minus the verification gate,
in a language with a smaller maintenance burden and served over MCP to any editor. If Air's ledger
ever needs replacing rather than trimming, read this first.

### 3.4 aGiTrack
Apache-2.0. <https://github.com/core-aix/agitrack> (accessed 2026-08-24).
Turns each agent turn into a git commit carrying an interaction trace plus metadata: backend, model,
input/output/cache/reasoning token counts, timings, session id, compaction events, subagent token
use. Sandboxes agent writes with `sandbox-exec` on macOS and Linux, confining them to
`.agitrack/worktrees/`. Explicitly **does not gate or refuse**: it records.
*Overlap with Air*: the provenance half of the ledger, at a finer grain than Air records, using git
as the store instead of SQLite. Worth stealing the idea that the commit *is* the record.

### 3.5 codecast
TypeScript, MIT, self-hosted Convex backend. <https://github.com/codecast-sh/codecast> (accessed
2026-08-24). Background daemon watches local agent history files (Claude Code, Codex, Cursor,
Gemini) and keeps a permanent searchable record. Triage inbox orders sessions Pinned → Working →
Needs Input → Idle → Deferred. `cast blame` is a git-blame replacement showing which agent session
wrote each line. Secrets redacted before sync, project paths hashed.
*Overlap with Air*: `air status --attention` and the capture inbox, built as a product. The
line-level attribution is something Air does not have and probably should not build.

### 3.6 paperclip
Node/React/TypeScript, MIT, PostgreSQL. <https://github.com/paperclipai/paperclip> (accessed
2026-08-24). Agents as employees: org charts with roles, reporting lines, permissions and budgets;
DB-backed wakeup queue with coalescing; atomic ticket checkout with execution locks; blocker
dependencies, comments, documents, work products, inbox state; token and cost tracking by company,
agent, project, goal, issue, provider and model with warning thresholds and hard stops; enforced
approval gates with revisioned config and rollback.
*Overlap with Air*: leases, heartbeats, claims and the human decision queue. No automated proof of
green before a ticket closes, which is the same gap as everywhere else.

### 3.7 Archon
TypeScript on Bun, MIT. <https://github.com/coleam00/archon> (accessed 2026-08-24).
Workflow engine: YAML DAGs mixing deterministic nodes (bash, tests, git) with AI nodes (plan,
implement, review). Loop nodes iterate until a condition holds, optionally with a fresh context each
iteration. Interactive approval gates pause for a human. Every run gets its own git worktree.
SQLite or Postgres, 14 tables covering codebases, conversations, sessions, workflow runs, isolation
environments and messages. No evidence linkage between test results and commits documented.
*Overlap with Air*: closest thing to "encode the procedure" as a product. Note that Air deliberately
does not encode the procedure (`docs/plans/0002`), so this is a road not taken, not a competitor.

### 3.8 kodo
Python 3.13+, MIT. <https://github.com/ikamensh/kodo> (accessed 2026-08-24).
Orchestrator (cheap model) directs Claude Code workers through work cycles; separate architect and
tester agents independently review before work is accepted, rejecting across multiple rounds. State
is files: cycle checkpoints for resume, run history JSON in `~/.kodo/runs/`, coverage notes in
`.kodo/test-coverage.md`. README claims 24% more real-world GitHub issues solved than single-agent
(vendor claim, not independently verified).
*Overlap with Air*: verification by a second agent rather than by recorded fact. Different bet:
kodo trusts a reviewer model, Air trusts a recorded exit status. Air's is cheaper and narrower.

### 3.9 Ivy-Tendril
Source-available, FSL-1.1-ALv2. <https://github.com/Ivy-Interactive/Ivy-Tendril> (accessed
2026-08-24). Plan-based lifecycle: plans created manually or from GitHub issue webhooks, executed by
any CLI agent in isolated worktrees, with inline annotation of drafts feeding revised goals back
into the plan, automated verification gates during review, and human approval before merge. The
public docs do not specify what the verification gates check or what happens on failure.
*Overlap with Air*: human checkpoints plus gates, but unspecified. Recheck on next refresh.

### 3.10 OpenClaw and its ecosystem
Docs <https://docs.openclaw.ai/tools/subagents> (accessed 2026-08-24).
OpenClaw is a personal-assistant harness (gateway, sessions, memory, triggers, tool loop), not a
coding fleet manager. Its subagents run in dedicated sessions `agent:<id>:subagent:<uuid>` with
isolated clean transcripts by default, optionally on a cheaper model, and **announce** their result
back to the requester with runtime stats and status. Limits: max nesting depth 5 (depth 2
recommended, where depth-1 agents review depth-2 workers), 8 active children globally, 5 per
session, configurable timeouts, sessions auto-archived 60 minutes after completion. Parents use
`sessions_yield` to wait rather than poll.
Around it: **corellis** (multi-agent governance for 20+ agent OpenClaw fleets: goal decomposition,
fleet-wide memory, correction propagation, approval workflows,
<https://github.com/CorellisOrg/corellis>) and **ClawTeam** (agents spawn and manage teammates,
file-based or P2P inboxes across tmux worktrees, <https://github.com/HKUDS/ClawTeam>), both accessed
2026-08-24 via the roster and secondary summaries, neither read in depth yet.
*Overlap with Air*: none on the gate. OpenClaw has no opinion about whether work is done. It is a
layer Air could sit *inside*, not a replacement.

### 3.10b AgentOps and agentic-os (found by the 2026-08-25 deep-research pass, not in the roster)

Neither is in the awesome-list roster, and between them they are the closest anything has come to
Air's gate.

**AgentOps** (boshu2/agentops, Apache-2.0, 428★, created 2025-11-05, pushed 2026-08-25;
<https://github.com/boshu2/agentops>, accessed 2026-08-25). Skill-driven plan/implement/validate
loop with a section headed **"Intent lives in a bead"**: beads is the preferred tracker, plan writes
BDD acceptance criteria and DDD ubiquitous language into the bead, implement builds against it
(TDD), and validate judges a **hashed snapshot** under `.agents/ao/intents/sha256/`. Ships a
dedicated `beads` skill. Works with or without beads, because the snapshot is of bytes.
*Read against Air*: the second independent third-party adoption of bd, and the only project found
that puts acceptance criteria into the tracked item and then judges against a recorded artifact.
Its evidence is content-addressed to filesystem state rather than bound to a commit, which is the
remaining difference from Air's "green recorded at a HEAD containing main".

**agentic-os** (KbWen/agentic-os, <https://github.com/KbWen/agentic-os>, accessed 2026-08-25).
Not a runner: it installs by copying Markdown, Bash, Python and PowerShell into a target repo
(`installers/deploy_brain.sh /path/to/your-project`), model-agnostic across Claude Code, Codex,
Cursor, Copilot and Antigravity, with no launcher, supervisor or daemon in the tree. What it does
have is a real enforcement point: `validate.sh` reads a work trail and fails when a required phase
was skipped or its evidence is missing, wired as an opt-in pre-commit hook and in CI, blocking a
ship with no review or test evidence. The gated unit is a phase of **its own** Markdown workflow
keyed to a branch (one branch, one owner; per-task log at `.agentcortex/context/work/<branch>.md`),
and the repo mentions no external tracker at all.
*Read against Air*: strongest match on mechanism, weakest on scope. It proves the pattern
"evidence in a trail, refusal at a hook" is not unique to Air; it also shows how far short that
falls of refusing a close on an externally tracked item.

### 3.11 gastown
<https://github.com/gastownhall/gastown>. Coordinator, git-backed issue tracking, health watchdogs,
Bors-style merge queue, 20 to 30 agents. Already deep-dived in
[`beads-and-gastown.md`](beads-and-gastown.md) §2.5 and used in this repo as the cautionary case for
building machinery without a named failure behind it. No re-derivation here.

### 3.12 Resting projects worth watching
From the roster's "Resting" section (no push in months, checked by the list author 2026-07-28):
**swarm-protocol** (<https://github.com/phuryn/swarm-protocol>, headless coordination over MCP:
claim work, detect file conflicts, heartbeat, hand off across sessions) and **gnap**
(<https://github.com/farol-team/gnap>, git-native agent protocol coordinating through a shared repo
as a task board with no orchestrator process). Both are the same idea as Air's coordination layer
with less machinery. If either revives, read it before adding anything to Air's.

---

## 4. The harness layer

Not previously covered in this repo, and the thing the owner flagged as missing.

**Definition in current use**: Agent = Model + Harness, where the harness is the code around the
model endpoint that manages context, tools, permissions, verification and the loop. The term is
tracked to Mitchell Hashimoto, February 2026, by several secondary sources; treat the attribution as
unverified until a primary is found.

**What the harness now does natively**, which is the number that matters for Air's scope: Claude
Code exposes worktree isolation, subagents, a deterministic workflow runner, hooks at
SessionStart/PreToolUse/PostToolUse/Stop/SessionEnd, MCP servers, skills, scheduled and background
runs, and a permission model with deny rules (observed 2026-08-24 in this session; catalogued in
[`claude-code-control-surfaces.md`](claude-code-control-surfaces.md)). Every one of those was, at
some point, something a third-party orchestrator sold. The trend line is the argument for keeping
Air's surface as small as possible: **assume anything Air does that is not evidence-keeping will be
absorbed within two release cycles.**

**Curated entry point for this layer**:
<https://github.com/ai-boost/awesome-harness-engineering> (accessed 2026-08-24), sections
Verification & CI Integration, Human-in-the-Loop, Memory & State, Permissions & Authorization.

---

## 5. Evidence about what actually matters

### 5.1 The harness dominates the model
"Stop Comparing LLM Agents Without Disclosing the Harness", Zhang, Wang, Ge, Xu, Hamm, Reddy, arXiv
2605.23950, submitted 2026-05-07 (<https://arxiv.org/abs/2605.23950>, accessed 2026-08-24). Proposes
the "Binding Constraint Thesis": performance variance is governed more by harness configuration than
by model choice, with "harness-induced variance [that] can substantially exceed model-induced
variance, including cases of model ranking reversal". Recommends a disclosure standard and a
variance decomposition protocol; treats leaderboard comparisons of long-horizon agents as incomplete
without harness transparency. **The abstract gives no headline number**; see §5.4.

### 5.2 What people actually configure
"Harness Engineering for Agentic AI Coding Tools: An Exploratory Study", Galster, Mohsenimofidi,
Lulla, Abubakar, Treude, Baltes, arXiv 2602.14690, submitted 2026-02-16, final 2026-06-30
(<https://arxiv.org/abs/2602.14690>, accessed 2026-08-24). Analyses configuration mechanisms across
Claude Code, Copilot, Cursor, Gemini and Codex, then 2,853 GitHub repositories. Findings: context
files dominate, with AGENTS.md emerging as an interoperable cross-tool standard; skills and
subagents are rarely adopted and skills are mostly static instructions rather than executable code;
Claude Code users adopt the broadest range of mechanisms.
*Relevance to Air*: the field's revealed preference is prose in a context file, which is exactly the
thing this repo's "machinery over Markdown" rule says loses. Air's bet is against the median
repository here. Worth stating plainly rather than assuming.

### 5.3 Humans on the loop, not in it
Martin Fowler, "Humans and Agents in Software Engineering Loops"
(<https://martinfowler.com/articles/exploring-gen-ai/humans-and-agents.html>, accessed 2026-08-24
via the harness-engineering list): three postures, human outside / in / on the loop, with the
argument that "humans on the loop", maintaining the harness rather than reviewing individual
outputs, is the only posture that scales with agent throughput. Anthropic, "Measuring AI Agent
Autonomy in Practice" (February 2026, <https://www.anthropic.com/news/measuring-agent-autonomy>,
accessed 2026-08-24 via the same list): across millions of real Claude Code interactions,
experienced users move from per-action approval (about 20% auto-approve when new) to
intervention-only oversight (about 40% auto-approve past 750 sessions), and agent-initiated
clarification stops grow faster than human interruptions as tasks get harder.
*Relevance to Air*: this is a direct challenge to the "a human is always in the loop" core
requirement as literally worded. The measured trajectory of skilled users is toward *on* the loop.
Air's actual mechanisms (a watchable terminal, `air status`, the event stream, a gate that refuses)
are on-the-loop mechanisms already; the prose is stricter than the machinery. Candidate rewording,
not a change to make during a round.

### 5.4 Correction
On 2026-08-24 the assistant told the owner that "swapping the harness changes SWE-bench scores by 22
points while swapping the model changes them by 1 point", sourced from
<https://amux.io/guides/harness-engineering/>. That figure did not survive a check against primary
sources and should not be used. What is supportable: §5.1's variance claim, and a reported spread of
about 9.5 points on SWE-bench when holding Claude Opus 4.5 fixed and varying only the harness (SEAL
versus Claude Code), which itself comes from a secondary analysis
(<https://www.digitalapplied.com/blog/swe-bench-verified-june-2026-benchmark-vs-scaffolding-analysis>,
accessed 2026-08-24) and is still unverified against a primary. The direction of the finding is well
supported; the magnitude is not. Recorded here because the claim crossed into the owner's decision
input, which is exactly the case the cross-project checking rule in `CLAUDE.md` covers.

Also worth reading for the same question: "Claw-SWE-Bench: A Benchmark for Evaluating OpenClaw-style
Agent Harnesses on Coding Tasks", arXiv 2606.12344, and "From Question Answering to Task Completion:
A Survey on Agent System and Harness Design", arXiv 2606.20683. Neither read yet.

---

## 6. Research areas and the refresh procedure

Where to look, what to take from each, how often. This is the part that keeps the document alive.

1. **The orchestrator roster**: <https://github.com/andyrewlee/awesome-agent-orchestrators>. Diff
   the README against the previous snapshot; new entries go to Appendix A unverified, entries that
   score on axes 1 to 4 get promoted into §3. Monthly. Snapshot 2026-08-24: 186 active, 17 resting.
2. **The harness-engineering roster**: <https://github.com/ai-boost/awesome-harness-engineering>.
   Read Verification & CI, Human-in-the-Loop, Memory & State, Permissions. Monthly.
3. **Harness papers**: arXiv cs.SE and cs.AI for "agent harness", "scaffold", "agentic coding".
   Known open reads listed at the end of §5.4. Monthly.
4. **Benchmarks that treat the harness as a variable**: SWE-bench Verified/Pro, Terminal-Bench,
   Claw-SWE-Bench. Only useful for the harness-versus-model question, not for feature comparison.
   Quarterly.
5. **First-party changelogs**, highest priority of all because this is where Air's surface gets
   eaten: Claude Code releases and docs, Claude Agent SDK, Codex CLI, OpenCode, Gemini CLI. Every
   release. Trigger: any release that adds orchestration, task state, or verification surfaces
   forces a same-week re-read of §2.1.
6. **OpenClaw and the assistant-harness layer**: <https://docs.openclaw.ai>, plus corellis and
   ClawTeam. Relevant only if the owner ever wants the always-on, wake-on-schedule, message-from-
   phone posture. Quarterly.
7. **The named neighbours**, by watching their releases rather than re-reading their READMEs:
   loki-mode, tutti, orc, Zaivern Code, bernstein, MartinLoop, guild, Factory, multica, 5dive,
   aGiTrack, paperclip. These are the ones that could make Air's residue redundant. Monthly. Watch
   contributor count and last-push date as well as features: on 2026-08-24 the closest four had 6, 3,
   1 and 2 contributors respectively, so their features matter less than whether they survive.
8. **Targeted GitHub search** for the residue itself, since a project doing exactly Air's thing
   would not be findable by category: queries such as `"verify" "close" gate agent commit sha`,
   `agent "proof of work" tests green before close`, `beads OR "issue tracker" agent evidence gate`.
   Quarterly, and any time §2.3 is about to be repeated as a claim.

9. **The unassessed alternatives to `bd`**: guild, swarm-protocol and gnap as claim-capable durable
   stores. The 2026-08-25 pass produced no verified claim about any of them, so Air's most exposed
   dependency has no measured competitor. Answer this before the next decision about the claim
   layer.
10. **The two off-roster neighbours** (§3.10b), AgentOps and agentic-os, by release rather than by
   README. AgentOps is the one to watch: it already puts acceptance criteria into a bead, and
   commit-binding its snapshot is a small step from where it is.

**Refresh procedure**: re-run 1, 2, 5 and 7; update §2 first and the entries second, because the
verdict is the product and the entries are the working; record what changed and what did not in a
dated line at the bottom of this section; if §2.3 shrinks to nothing, say so loudly, because that is
the finding that would retire a component of Air.

**Refresh log**
- 2026-08-25: targeted deep-research pass (108 agents, 25 primary sources, 124 claims extracted, 25
  adversarially verified, 9 killed). herdr re-read first-hand and re-classified from Commodity to
  the fleet layer worth renting; two projects added that the roster does not carry (AgentOps,
  agentic-os), both closer to Air's gate than anything in the 186. One error corrected: this doc
  had said herdr has no worktree management, and it has had it since 0.6.2. Left unanswered: guild,
  swarm-protocol and gnap as claim-capable bd alternatives, which is now watch item 9.
- 2026-08-24: created, then extended the same day from 11 projects read to all 186. The extension
  changed the verdict: §2.2 went from three overlapping projects to eleven, and four projects found
  only in the full sweep (loki-mode, tutti, orc, Zaivern Code) are closer to Air's core than
  anything in the first pass. §2.3 survived, narrowed to three properties. §2.5 added with the
  adopt-versus-keep numbers. Lesson for the next refresh: the shallow pass was wrong in the
  direction of flattering this repo, and the projects that mattered most had 7, 23 and 112 stars.

---

## 7. What would change the verdict

- A project that gates a **tracked work item** (not a run) on recorded evidence, in an interactive
  session. That is §2.3 gone; Air's remaining reason to exist would be the bd integration.
- Claude Code shipping a first-party "task did not verify" refusal, or hooks gaining access to
  structured verification state. Watch item 5.
- **tutti** growing past three contributors, or **orc** past one. Either would be Air's architecture
  with a maintainer base, and adoption would beat re-derivation. Watch item 7.
- **loki-mode** relicensing from BUSL-1.1 to something permissive. Its receipt format is better than
  Air's and the license is the only thing making that awkward.
- guild adding verification, or MartinLoop adding a durable work store. Either one closes the gap
  from its side. Watch item 7.
- The owner deciding the fleet matters more than the gate, in which case the right move is to adopt
  scion or herdr for the fleet and keep only the hook and the ledger (§2.5, position 2).

---

## Appendix A: every rostered project, with notes

Source of the roster: <https://github.com/andyrewlee/awesome-agent-orchestrators>, snapshot
2026-08-24 (186 active, 17 resting). Stars, language, license and last-push date come from the
GitHub REST API on 2026-08-24. Notes come from each project's own README read on that date, so they
are the project's claims about itself, not a test of those claims. One repository in the roster
(`ariana-dot-dev/ariana`) returns 404 and is omitted.

**Verdict tags**
- **Commodity**: does something Air also does and the harness is absorbing. Evidence to delete, not
  to adopt.
- **Neighbour**: touches Air's residue (evidence, gates, durable work items, claims). Read before
  changing that part of Air.
- **Watch**: interesting mechanism, not yet a reason to act.
- **Out of scope**: assistant, GUI, or platform play; not about proving work finished.
- **Dormant**: no push in months.

### A1. Multi-agent swarms (25)

The category closest to Air's shape. Three of these are ahead of Air on the thing Air treats as its
own.

**loki-mode** (asklokesh/loki-mode) · 1,047★ Shell BUSL-1.1 · pushed 2026-08-24 · **Neighbour, the
closest one found.** Spec-driven builder whose stated premise is "it does not accept 'done' on an
empty diff or failing tests". Every run writes an Evidence Receipt to `.loki/proofs/<run_id>/` that
deliberately separates **machine facts** (the command, its exit code, each gate verdict) from **AI
assessments** (a council verdict, labelled as judgment and never as proof). The headline is computed
from the facts: `VERIFIED` requires that tests ran a real command, exited 0, the diff is non-empty
and nothing was suppressed; otherwise `VERIFIED WITH GAPS` with each gap named. The diff hash is
recomputable by a third party who never installed the tool. 41 agents, 8 swarms, nine quality gates,
blind three-reviewer review. Source-available, not open source.
*Read against Air*: this is Air's "proof is a command and its output, not a description" rule, built
out further than Air has built it, and with a facts-versus-judgment split Air does not have and
should steal. What Air still has that this does not: the refusal is attached to a tracked work item
(`bd close`) rather than to a run the tool itself orchestrated, and the green must sit at a HEAD
containing main.

**tutti** (nutthouse/tutti) · 112★ Rust MIT · pushed 2026-07-28 · **Neighbour.** "Terraform-style
agent operations": `tutti.toml` declares roles, runtimes and workflows; the loop is intake → run →
review → **gate** → **record**. Gate means required checks, resolved review threads, approval state
and cost limits. Record means a run ledger plus artifacts, logs and replayable state. Has issue
claim leases with `tt issue-claim acquire|heartbeat|release|sweep`, spawns agents in tmux with a
worktree each, and writes verify output to `.tutti/state/verify.json`.
*Read against Air*: the same seven nouns as Air (claim, lease, heartbeat, worktree, verify, gate,
ledger), in the same language, one month older. If Air were being started today this is the project
to fork or contribute to rather than to re-derive. Its gate is a PR/checks gate, not a
recorded-green-at-a-merged-HEAD gate, which is the remaining daylight.

**5dive** (5dive-ai/5dive) · 54★ Shell MIT · pushed 2026-08-24 · **Neighbour.** A company of agents
on one box with no heavy runtime: bash, SQLite and systemd, comfortable on a 1 GB VM. Shared task
queue in SQLite, every row carrying **its assignee, its verifier, and where the maker-to-verifier
handoff has got to**. Heartbeats wake an assignee only when there is work. Browser views for org
chart, task queue and human gates. Durable memory search with provenance. Adversarial multi-agent
review on demand.
*Read against Air*: the maker/verifier split as a field on the work item is a cleaner encoding of
Air's hand-over than Air's own, and the resource floor is a reminder of how little machinery this
needs.

**ORCH** (oxgeneral/ORCH) · 147★ TypeScript MIT · pushed 2026-08-01 · **Watch.** Typed teams driven
by an explicit state machine; worktree per agent; "nothing touches main until reviewed" as a
mandatory review gate in that state machine; zombie detection and retry with backoff; no database,
no Docker. Review is by agent, not by recorded evidence.

**Fusion** (Runfusion/Fusion) · 1,155★ TypeScript MIT · pushed 2026-08-24 · **Commodity+.** Kanban
plus graph view over tasks, per-task worktree and branch, plan → review → execute → review per step,
an agent mailbox with Inbox/Outbox/Approvals, hierarchical missions across multiple nodes. Polished,
large, and entirely about routing work rather than proving it done.

**orc** (spencermarx/orc) · 23★ Shell (no license file) · pushed 2026-06-21 · **Neighbour, and the
most direct vocabulary collision.** Built on **beads (`bd`)** for work tracking, with a three-tier
orchestrator (root, project, goal) that "coordinates work but never writes code", decomposition of a
goal into **beads**, one engineer per bead in its own worktree, an ephemeral reviewer per bead, and
automatic review before merge to the goal branch. `orc add` runs `bd init` and hides `.beads/`,
`.worktrees/`, `.goals/` via `.git/info/exclude`.
*Read against Air*: same task store, same nouns, same coordinator/worker split, same worktree rule,
arrived at independently. It has no ledger and no evidence gate: review is an agent's opinion. That
is precisely the gap Air fills, which is the clearest statement yet of what Air is for.

**ClawTeam** (HKUDS/ClawTeam) · 5,518★ Python MIT · pushed 2026-05-09 · **Commodity.** Leader agent
calls `clawteam spawn`; each worker gets a git worktree, a tmux window and an identity; tiled tmux
or web UI to watch; "you intervene only when you want to". Air's launcher layer with more stars.

**corellis** (CorellisOrg/corellis) · 28★ Shell MIT · pushed 2026-04-13 · **Out of scope.** Fleet
governance for OpenClaw: a controller on the host manages one Docker container per person, each with
private memory plus shared read-only company knowledge, Slack app provisioning per agent. Governance
here means approvals and shared knowledge files, not verification.

**gastown** (gastownhall/gastown) · 17,758★ Go MIT · pushed 2026-08-19 · **Neighbour, already
studied.** Coordinator, git-worktree-backed persistent work tracking that survives crashes, beads
integration, health watchdogs, Bors-style merge queue, Docker or host install. Deep dive already in
[`beads-and-gastown.md`](beads-and-gastown.md) §2.5; this repo's standing verdict is that it is the
cautionary case for machinery without a named failure behind it.

**agent-kanban** (saltbo/agent-kanban) · 456★ TypeScript · pushed 2026-08-22 · **Commodity.** Leader
plus daemon dispatching workers, each in its own worktree; workers claim, implement and open PRs;
SQLite behind a Hono API; agent lifecycle idle → working → offline and task flow Todo → In Progress
→ In Review → Done. Cryptographic agent identity is the one unusual bit.

**AgentsMesh** (AgentsMesh/AgentsMesh) · 2,327★ Go · pushed 2026-08-03 · **Commodity.** "AgentPod" =
PTY + worktree sandbox + output stream; tickets on a kanban board bound to pods with PR tracking;
Docker Compose, Postgres, multi-machine. Enterprise-shaped.

**buzz** (block/buzz) · 30,429★ Rust Apache-2.0 · pushed 2026-08-24 · **Watch, unusual.** Every
message, reaction, workflow step, review approval and git event is a signed Nostr event in one log,
with agents holding their own keys and their own audit trail. A feature branch becomes a room where
patches, CI, review and the merge decision live together.
*Read against Air*: the only project in the sweep where the audit trail is cryptographically
attributable per actor. Overkill for one owner and three workers, but the identity-not-permission-
flags framing is worth remembering if Air ever spans machines.

**Agon** (AutoResearch-Factory/Agon) · 42★ Python MIT · pushed 2026-08-22 · **Out of scope.** Claude
Code plugin for autonomous research loops (idea → literature check → experiment), with an arXiv
paper behind it. Not software delivery.

**agent-teams-ai** (777genius/agent-teams-ai) · 1,980★ TypeScript AGPL-3.0 · pushed 2026-08-24 ·
**Commodity.** Kanban where agents talk to each other, create tasks, review and comment; per-teammate
choice of main checkout or worktree at launch; configurable autonomy from full-auto to per-tool
approval; auto-resume after rate limits.

**claude_codex_bridge** (SeemSeam/claude_codex_bridge) · 3,445★ Python · pushed 2026-08-20 ·
**Commodity.** Cross-provider TUI where a config line declares the team
(`work = "worker1:codex(worktree), worker2:claude(worktree)"`, `review = "reviewer:claude, qa:gemini"`)
and `/ask reviewer ...` addresses one of them. Mobile viewport reflow of tmux panes.

**CompanyHelm** (CompanyHelm/companyhelm) · 74★ TypeScript MIT · pushed 2026-06-29 · **Commodity.**
Control plane running each agent session in a dedicated isolated environment, model-agnostic across
five provider types, Postgres plus Redis plus Docker Compose.

**hcom** (aannoo/hcom) · 461★ Rust MIT · pushed 2026-08-09 · **Watch.** Agents spawn, fork, resume
and kill each other across any terminal emulator; hooks record activity to a local SQLite database
and deliver messages from it; every agent runs in a real terminal you can see, scroll and interrupt.
*Read against Air*: the closest thing to Air's "every agent session is a terminal the owner can
watch" requirement, implemented as a messaging layer rather than a launcher.

**multi-agent-shogun** (yohey-w/multi-agent-shogun) · 1,418★ Shell MIT · pushed 2026-08-06 ·
**Commodity.** Shogun/karo/ashigaru hierarchy over tmux, 10 agents, every instruction and report a
plain YAML file you can diff and version-control, phone control via Termux.

**forge-orchestrator** (nxtg-ai/forge-orchestrator) · 158★ Rust · pushed 2026-08-20 · **Watch.**
Single Rust binary, no daemon and no database: state is files under `.forge/`, Claude Code talks to
it over MCP stdio while Codex and Gemini use filesystem conventions. Adds file locking, knowledge
capture, task planning and drift detection. Its framing is that multi-agent *within* one tool is
solved and the real problem is multi-*tool* on a shared repo.

**Orkas** (Orkas-AI/Orkas) · 1,423★ TypeScript MIT · pushed 2026-08-24 · **Out of scope.** Desktop
"commander decomposes to specialists" app, bring-your-own-keys across nine providers.

**paperclip** (paperclipai/paperclip) · 79,304★ TypeScript MIT · pushed 2026-08-24 · **Neighbour,
already in §3.6.** Note the scale: the most-starred coding-fleet manager in the roster, with org
charts, heartbeats, atomic ticket checkout, budget hard stops that pause agents on overspend, and an
immutable audit log. Still no proof-of-green before a ticket closes.

**ruflo** (ruvnet/ruflo) · 69,270★ TypeScript MIT · pushed 2026-08-24 · **Out of scope, but note the
size.** "Meta-harness" exposing swarms through CLI and MCP with 27 hooks. Enormous mindshare,
generic swarm framing, nothing about verified completion.

**scion** (GoogleCloudPlatform/scion) · 1,683★ Go Apache-2.0 · pushed 2026-08-24 · **Commodity, but
the best-built one.** Container per agent plus optional worktree and separated credentials;
harness-agnostic with Gemini CLI and Claude Code shipped and others as opt-in bundles; agents run in
tmux for attach/detach with message enqueue while detached; Docker, Podman, Apple containers and
Kubernetes profiles. Google-published.
*Read against Air*: if isolation ever needs to be stronger than a worktree, adopt this rather than
build it.

**shire** (victor36max/shire) · 38★ TypeScript MIT · pushed 2026-05-03 · **Watch.** Persistent team
workspaces with inter-agent mailboxes and a shared drive, SQLite at `~/.shire/`, local processes
only. Framed as "agents that work with you, not for you" against the fire-and-forget pattern.

**kodo** (ikamensh/kodo) · 128★ Python MIT · pushed 2026-07-18 · **Neighbour**, written up in §3.8.

### A2. Parallel coding agents, terminal (15)

Uniformly commodity. This is the category the harness has most obviously absorbed: every one of
these is a way to see and resume sessions that Claude Code now starts, isolates and resumes itself.
Listed for completeness and to make the point concrete.

**agent-console** (buhuipao/agent-console) · 16★ Rust Apache-2.0 · 2026-08-19 · **Commodity.**
Discovers Codex and Claude Code sessions from the providers' own transcript files, including ones
started elsewhere, and resumes the native UI rather than replacing it. No tmux, no worktrees. The
"read the provider's own state instead of owning state" approach is the cheapest design here.

**agent-deck** (asheshgoplani/agent-deck) · 784★ Go MIT · 2026-08-24 · **Commodity.** One TUI over
Claude, Gemini, OpenCode and Codex; attaches MCP servers per project without editing config files
and restarts the session for you; waiting sessions surface in the tmux status bar.

**agent-manager** (YoanWai/agent-manager) · 351★ Go Apache-2.0 · 2026-08-24 · **Commodity.** Eight
agent CLIs side by side, each in its own persistent tmux session; optional worktree per session
(`am/` branches); in-terminal diff review that sends line comments back to the agent. End-to-end
tests drive a real tmux server, which is unusual diligence for the category.

**agent-of-empires** · 3,127★ Rust MIT · 2026-08-24 · **Commodity.** TUI plus browser view of the
same sessions for phone access; Docker/Podman/Apple-container sandboxing with shared auth volumes;
diff review in the TUI.

**agentbox** (madarco/agentbox) · 374★ TypeScript MIT · 2026-08-24 · **Watch (isolation only).** One
sandboxed VM per agent, local Docker or Daytona/Hetzner/Vercel/E2B/DigitalOcean, with prepared
snapshots for sub-second starts and a remote-docker mode over plain SSH.

**agterm** (umputun/agterm) · 497★ Swift MIT · 2026-08-23 · **Commodity.** Native macOS terminal,
named workspaces, agent-status colours, full control API, pushes to agents in remote tmux over SSH.

**amux** (andyrewlee/amux) · 151★ Go MIT · 2026-08-24 · **Commodity.** Minimal worktree-per-agent
TUI by the author of the roster itself. Note one design detail worth copying nowhere: it runs git
with repository hooks disabled, so a project's own pre-commit hooks do not fire on its actions.

**claude-squad** (smtg-ai/claude-squad) · 8,361★ Go AGPL-3.0 · 2026-08-20 · **Commodity.** Detached
background session per agent with its own workspace, so work continues after the pane closes;
review and checkout changes before pushing.

**cmux** (manaflow-ai/cmux) · 26,410★ Swift · 2026-08-24 · **Commodity.** Ghostty-based macOS
terminal with vertical tabs and per-agent notifications; runs Claude Code's own teammate mode as
native splits with no tmux involved, which is the harness-absorbs-the-category trend in one line.

**dmux** (standardagents/dmux) · 1,750★ MIT · 2026-08-16 · **Commodity.** Pane per agent, worktree
and branch created for you, auto-commit plus merge plus cleanup in one step.

**herdr** (**herdrdev/herdr**, the roster's `ogulcancelik/herdr` redirects) · 32,266★ Rust
Apache-2.0 · created 2026-03-27, pushed 2026-08-24, **77 contributors**, v0.8.2 on 2026-08-19 with
daily preview builds · **Adopt candidate for the fleet layer.** Read first-hand 2026-08-25, after
the roster line ("persistent workspaces, tabs, panes, status detection") proved to be an
undersell.

A terminal workspace manager: workspaces hold tabs, tabs hold panes, panes hold real processes that
survive client detach. Agents are classified `blocked` / `working` / `done` / `idle` / `unknown`,
detected from foreground processes, screen manifests and optional integrations across 16+ agent
CLIs. The part that matters is the **socket API** (newline-delimited JSON over a Unix socket or a
named pipe): create/list/focus/rename/close workspaces, tabs and panes; `list, inspect, read,
prompt, wait on, rename, focus, start, and attach` agents; **`herdr agent wait <pane> --until
done`**; **event subscriptions** that stream workspace, tab, pane and agent-status changes on a
long-lived connection; notifications; input into panes; and **`pane.report_agent`**, which lets an
integration report its own semantic state back into herdr's agent awareness.

It **does** manage git worktrees natively (`herdr worktree list|create|open|remove`, CLI and socket
API, since 0.6.2 on 2026-05-23, with sidebar worktree groups): an earlier draft of this entry said
otherwise on the strength of one docs page that did not mention them, and the changelog says
otherwise.

What it does **not** have, checked against Air's axes across the ~75-method socket API, the
1,099-line changelog, the docs index and `AGENTS.md`: no task store, no work queue, no ticket
tracking, no verification-run recording, no commit-bound evidence, no gate or refusal, no approval
workflow, no MCP, no tracker integration. Zero changelog hits for budget, spend, tracker, beads,
ledger or sqlite. Plugins "that need durable state should own their files or database" by design.
Its agent `done` is self-reported or screen-inferred, so it can feed an idle signal and never a
gate. It also does not inject role prose or hold a permission/deny list, which is the part of Air's
launcher that would survive adoption. Licence verified from the LICENSE file (Apache-2.0; some
third-party writeups claim AGPL-3.0, and star counts in circulation range 15k to 32.3k).
*Read against Air*: this is not a competitor, it is the missing half. herdr is the fleet layer Air
should rent (panes, persistence, liveness, remote access, an event stream to replace Air's poll
thread), and it has no opinion whatever about whether work is finished, which is the half Air
keeps. `pane.report_agent` means Air's conditions can surface in herdr's UI instead of in a screen
Air has to render. See [`../plans/0007-surface-audit.md`](../plans/0007-surface-audit.md) §10.5.

**openkanban** (TechDufus/openkanban) · 141★ Go AGPL-3.0 · 2026-06-12 · **Commodity.** Terminal
kanban where each ticket owns a git worktree and an embedded terminal.

**repomon** (AliHamzaAzam/repomon) · 16★ Rust Apache-2.0 · 2026-08-23 · **Watch.** Many repos ×
worktrees × agents on one screen, durable across restarts; a background daemon (`repomond`) owns
SQLite, file watchers and the git layer; bundles its own tmux; phone approval. The daemon-owns-SQLite
split is the same architecture as Air's ledger plus channel.

**thurbox** (Thurbeen/thurbox) · 48★ Rust MIT · 2026-08-24 · **Commodity.** Persistent tmux panes
per agent, SSH sessions, inter-session messaging, code-review view.

**tmux-ide** (wavyrai/tmux-ide) · 539★ TypeScript MIT · 2026-08-23 · **Watch, cheapest idea in the
category.** Does not replace anything: `tmux-ide adopt <session>` drops a status bar with fleet tabs
onto a tmux session you already have, gets ground-truth agent status from Claude Code hooks
(`tmux-ide integration install claude`), streams status transitions (`tmux-ide events --follow`),
and `unadopt` reverts because it was only ever tmux options.
*Read against Air*: the additive, reversible, hooks-for-truth shape is the model for anything Air
adds to an owner's environment.

### A3. Autonomous loop runners (11)

Drive one goal until it verifies. The interesting ones here treat "done" as something to be proven,
which is why two of them are Air's nearest neighbours despite taking the human out.

**bernstein** (sipyourdrink-ltd/bernstein) · 976★ Python Apache-2.0 · 2026-08-24 · **Neighbour**,
written up in §3.1. Two details worth adding after reading the README directly: receipts are
verified with a public key (`bernstein verify receipt run-receipt.json --public-key ...pem`) and the
audit chain is checkable after the fact (`bernstein audit verify` validates an HMAC chain plus a
Merkle seal). Its stated posture, "determinism is something you check, not something you take on
faith", is the same instinct as Air's gate applied to the orchestrator itself.

**MartinLoop** (Keesan12/martin-loop) · 45★ TypeScript Apache-2.0 · 2026-08-24 · **Neighbour**,
written up in §3.2. Its one-line pitch is the cleanest statement of the whole category: "Your coding
agent says it's done. MartinLoop makes it prove it." Ships an MCP server as well as a CLI.

**Dex** (francescoalemanno/dex) · 21★ Rust MIT · 2026-06-11 · **Watch.** Names the failure Air also
names: without structure "the agent decides when it's done, retries are you hitting Ctrl+C, and
review is run it again and hope". Its answer is a plan you can amend plus a bounded review loop with
two reviewers at a time for up to three rounds, ending when both report zero issues or the cap hits.
Agent-judged, not evidence-judged.

**fractal** (plasma-ai/fractal) · 696★ Python Apache-2.0 · 2026-08-20 · **Watch.** Each node iterates
toward a goal in its own worktree and can spawn children, with depth and cost limits; agents run in
tmux; every run, iteration, step, cost and signal lands in one local SQLite database. A `scope`
setting restricts a node's commits to subdirectories.

**LoopTroop** (looptroop-ai/LoopTroop) · 123★ TypeScript MIT · 2026-08-24 · **Neighbour, note the
vocabulary.** Implements "only the Beads methodology, not the full external Beads Project",
crediting Steve Yegge: an epic is split into "beads", the smallest independently implementable
units. Planning is an LLM council that drafts, scores against a weighted rubric, votes, then refines
and verifies coverage. Execution runs each bead through auto-fix loops in isolated OpenCode
worktrees; failed final checks become QA-fix beads. Human approval gates at planning, execution
blueprint and final PR, described as becoming optional in future releases. State is SQLite plus
JSONL logs plus inspectable `.ticket/**` YAML.
*Read against Air*: a second independent adoption of beads-as-methodology, plus the honest admission
that their human gates are on the way out. Air's bet is the opposite.

**Loop Engineering** (cobusgreyling/loop-engineering) · 10,635★ JavaScript MIT · 2026-08-24 ·
**Watch.** Not a runtime: a pattern library plus `loop-init` and `loop-audit` CLIs that score a
repository's "loop readiness" and suggest improvements. Runs its own pattern-validation workflow on
every push.
*Read against Air*: the only project in the sweep that treats the harness setup itself as something
to audit and score. If Air ever wants a "is this repo ready for a fleet" check, start here.

**ralph-claude-code** (frankbria/ralph-claude-code) · 9,607★ Shell MIT · 2026-07-18 · **Watch.**
Ralph loop with a "dual-condition exit gate" requiring both completion indicators and an explicit
EXIT_SIGNAL, plus tmux monitoring. The gate is on the loop terminating, not on the work being right.

**ralph-orchestrator** (mikeyobrien/ralph-orchestrator) · 3,106★ Rust MIT · 2026-08-21 ·
**Commodity.** Hat-based Ralph implementation; runs as an MCP server over stdio scoped to a single
workspace root per instance.

**ralph-tui** (subsy/ralph-tui) · 2,426★ TypeScript MIT · 2026-05-13 · **Commodity.** Task-list
driver with a TUI, sandbox status indicator, session persistence with lock management, and JSONL
audit logging of all remote actions to `~/.config/ralph-tui/audit.log`.

**ralphex** (umputun/ralphex) · 1,446★ Go MIT · 2026-08-24 · **Commodity+.** Executes an
implementation plan autonomously with a fresh session per task, a five-agents-then-codex-then-two-
agents review pipeline with user-defined review prompts, automatic commits after each task and
review fix, Docker isolation, and `--worktree` for running plans in parallel.

**toryo** (JesseRWeigel/toryo) · 12★ TypeScript MIT · 2026-05-23 · **Watch, one good idea.** Plan →
research → execute → review, where a reviewer agent scores output 1 to 10 and a "ratchet" keeps or
reverts: `if (!ratchet.shouldKeep(review)) await ratchet.revert()`. Agents accrue trust levels that
change what they get delegated.
*Read against Air*: quality ratcheting (never accept a change that scores below the current bar) is
a mechanism Air does not have and could express cheaply as a ledger query.

### A4. Autonomous task runners (18)

Pull work from a tracker, board or schedule and run it unattended. The category where the durable
work item already exists, which makes the absence of evidence gates in most of them notable.

**Factory** (owainlewis/factory) · 202★ Go MIT · 2026-08-24 · **Neighbour.** "Run repeatable software
work through AI coding agents across repositories and machines." Every Attempt runs in its own
worktree; durable coordination is SQLite state plus **leases, heartbeats and retries**; a scheduler
with run admission; agent, worktree, result, failure and retry recorded in one place; embedded UI
with table, list and kanban views.
*Read against Air*: Air's lease and heartbeat layer, in Go, with a cleaner Attempt abstraction. No
evidence gate on completion.

**multica** (multica-ai/multica) · 47,492★ Go · 2026-08-24 · **Neighbour, and the one with real
adoption.** "Agents that show up on the board." Work is assigned the way you would assign it to a
person, across 23 agent CLIs, self-hosted via Docker Compose or Helm. Two claims matter: **"work
lands in review, not in main, you decide what ships"**, and "the intent, the run, the decisions and
the diff stay connected", with "an audit trail that includes the robots".
*Read against Air*: the closest thing to Air's philosophy with 47k stars behind it. Worth an hour of
real reading before Air adds anything to its own board or audit surface.

**Contrabass** (junhoyeo/contrabass) · 216★ Go Apache-2.0 · 2026-07-17 · **Neighbour.** Go and Charm
implementation of OpenAI's Symphony: issue-driven runs, worktree-provisioned workspaces under
`workspaces/`, a local task board, a **phased pipeline plan → exec → verify**, dual worker modes
(tmux multi-process or in-process goroutines), JSONL event logging, file-based heartbeats, a dispatch
queue and governance policies.
*Read against Air*: JSONL events plus file heartbeats plus a verify phase is Air's event stream and
gate, minus the ledger.

**symphony** (openai/symphony) · 26,830★ Elixir Apache-2.0 · 2026-08-19 · **Watch, already studied.**
Turns project work into isolated autonomous implementation runs so teams "manage work instead of
supervising coding agents", monitoring a Linear board and spawning agents. Covered in
[`prior-art-landscape.md`](prior-art-landscape.md) §A4.

**sortie** (sortie-ai/sortie) · 131★ Go Apache-2.0 · 2026-08-24 · **Commodity.** Single binary
turning tracker tickets into isolated agent sessions; agent-agnostic and tracker-agnostic.

**cyrus** (cyrusagents/cyrus) · 782★ TypeScript Apache-2.0 · 2026-08-23 · **Commodity.** Watches
Linear, GitHub, GitLab or Slack for issues assigned to it and spins an isolated worktree per issue,
then opens the PR. Deployable anywhere; tmux for detached running.

**lalph** (tim-smart/lalph) · 130★ TypeScript MIT · 2026-07-08 · **Commodity.** Issue-source-driven
orchestrator keeping task state in sync, with projects grouping concurrency, target branch, git flow
and whether a review agent runs; optional PR flow with auto-merge and issue dependencies.

**open-swe** (langchain-ai/open-swe) · 10,611★ Python MIT · 2026-08-24 · **Out of scope.** Framework
for building an org's internal coding agent, invoked from Slack, Linear or GitHub, each thread
getting a persistent sandbox across five sandbox providers, tasks parallel with no queuing.

**OpenHands** (OpenHands/OpenHands) · 84,961★ TypeScript MIT · 2026-08-24 · **Out of scope, note the
size.** Self-hosted control center running OpenHands, Claude Code, Codex, Gemini or any ACP agent
across local, remote and cloud backends, on schedules and webhooks. The largest project in the
roster after OpenClaw and Hermes.

**background-agents** (ColeMurray/background-agents) · 2,674★ TypeScript MIT · 2026-08-24 ·
**Commodity.** Control plane plus sandbox data plane, SQLite, WebSocket and event bus, per-repo
permission before a session is created, filesystem snapshots after each prompt so follow-ups restore
state.

**centaur** (paradigmxyz/centaur) · 1,178★ Python · 2026-08-24 · **Out of scope.** Slack-native
agent conversations with sandbox lifecycle, durable transcripts, credential injection and workflow
execution; Kubernetes-shaped.

**Taskuary** (ldbumble/taskuary) · 18★ Python MIT · 2026-08-24 · **Watch.** Local-first task hub
pulling email, Teams, Slack and scheduled reports into one timeline, with an agent kanban (Queued /
Working / Waiting on you / Done) ordered "by what is TRUE", a **Review decision queue** where you
approve and send, memory entries that cite why they are believed and when last seen, SQLite in
`~/.taskuary/`, and audit-chain verification.
*Read against Air*: the "Waiting on you" column plus a decision queue is Air's `--attention` surface
generalised beyond code.

**aeon** (aeonfun/aeon) · 683★ TypeScript MIT · 2026-08-24 · **Watch.** Runs unattended on GitHub
Actions across six harnesses with self-healing skills and quality scoring.

**gh-aw** (github/gh-aw) · 4,991★ Go MIT · 2026-08-24 · **Watch, first-party.** Compiles agentic
workflows written in Markdown into GitHub Actions YAML, routing MCP calls through a single gateway
for centralised access management. GitHub's own answer to the category.

**claude-code-action** (anthropics/claude-code-action) · 8,710★ TypeScript MIT · 2026-08-23 ·
**Out of scope (CI).** Anthropic's official Action: mode detection, PR review, custom review
checklists, MCP and permission configuration.

**codex-action** (openai/codex-action) · 1,207★ TypeScript Apache-2.0 · 2026-08-24 · **Out of scope
(CI).** OpenAI's official Action with explicit permission profiles for filesystem and network.

**run-gemini-cli** (google-github-actions/run-gemini-cli) · 2,059★ TypeScript Apache-2.0 ·
2026-08-21 · **Out of scope (CI).** Google's official Action; `@gemini-cli /review` on a PR.

**remote-swe-agents** (aws-samples/remote-swe-agents) · 241★ TypeScript MIT-0 · 2026-08-24 · **Out of
scope.** Serverless Lambda control plane with dedicated EC2 workers, as an AWS reference sample.

### A5. Agent infrastructure and primitives (19)

Control planes, coordination protocols, harness adapters. The category to read when deciding what
Air should *be*, as opposed to what it should *do*.

**guild** (mathomhaus/guild) · 313★ Go Apache-2.0 · 2026-08-18 · **Neighbour**, written up in §3.3.
One more detail from the README: "switching MCP clients requires no export, no migration", because
state is SQLite under `~/.guild/` and the client is just an MCP consumer. That is the portability
property Air's ledger should aim for.

**Crewplane** (crewplaneai/crewplane) · 34★ Python Apache-2.0 · 2026-08-24 · **Neighbour.** "Agents
do the work. You own the workflow." The whole process is defined in Markdown (prompts, stages,
agents, handoffs) and its stated purpose is the sentence Air would write about itself: **"Make review
a gate, not a promise buried in a prompt."** Completed nodes are validated so a failed workflow
resumes instead of restarting, every handoff artifact stays on disk, and it opens tmux when a
terminal is available.
*Read against Air*: same diagnosis (a prompt is not a guarantee), different remedy (validated
workflow nodes rather than a refusal at the moment of closing).

**Claudexor** (razzant/claudexor) · 423★ TypeScript MIT · 2026-08-24 · **Neighbour, one strong
idea.** Multi-harness control plane with quota-aware rotation across accounts, races that pit
harnesses against each other with cross-family review, and the design rule that **"every claim,
cost, quota, web evidence, auth route, is a typed fact"**, with honest accounting where unknown cost
is never recorded as `$0`.
*Read against Air*: "typed fact, and unknown is not zero" is exactly Air's ledger discipline stated
as a general principle. Steal the phrasing for `docs/`.

**LionClaw** (moshthepitt/lionclaw) · 15★ Rust MIT · 2026-08-06 · **Neighbour.** Local control plane
running real agent CLIs as **durable, auditable workers**: instances, runtime profiles, sessions,
audit records, installed skills, channels, scheduled work, policy-selected prompt context, plus
per-worker workspace access, network mode, install policy and secret mounts. Explicitly not a
wrapper that changes the agent: "Codex is still Codex."
*Read against Air*: the nearest Rust analogue to `air worker` plus the ledger, with a stronger
confinement story and no verification gate.

**aGiTrack** (core-aix/agitrack) · 14★ Python Apache-2.0 · 2026-08-24 · **Neighbour**, in §3.4. Extra
detail: manual mode records each turn on a hidden latent ref and folds it into *your* commit via
`prepare-commit-msg`, and the CLI surfaces a `merge` command at the top whenever a worktree still
holds un-integrated work (committed-but-unmerged or uncommitted). That un-integrated-work detector
is the same signal Air's hand-over gate computes.

**codecast** (codecast-sh/codecast) · 27★ TypeScript MIT · 2026-08-24 · **Neighbour**, in §3.5. Extra
detail: inline review over any assistant reply batched into one review, permission prompts answered
with Y/N from the inbox, scheduled follow-up work, and node-based workflow definitions that include
an explicit **human gate node**.

**Archon** (coleam00/Archon) · 23,266★ TypeScript MIT · 2026-08-24 · **Watch**, in §3.7. Note the
star count: by a wide margin the most adopted "harness builder", and its pitch, deterministic and
repeatable AI coding, is the same problem statement as Air's procedure.

**Agentlas OS** (agentlas-ai/Agentlas-OS) · 1,142★ Python Apache-2.0 · 2026-08-24 · **Watch.** Hub of
specialist agents with a temporary orchestrator spun up per task; explicit roles with managed
routing, handoffs and review boundaries; packages carry contracts, receipts and verification that
outlive the chat; a kernel/policy gate routes every call deterministically. Local-first.

**omnigent** (omnigent-ai/omnigent) · 9,228★ Python Apache-2.0 · 2026-08-24 · **Watch.** Meta-harness
giving one orchestration layer over Claude Code, Codex, Cursor, OpenCode, Hermes and Pi, with seven
sandbox providers, tmux terminal wrappers, and mandatory `bwrap` OS-sandboxing on Linux. Ask one
agent to review another's work.

**openfang** (RightNow-AI/openfang) · 18,134★ Rust Apache-2.0 · 2026-07-02 · **Watch, the Rust one.**
"Agent OS" in 137K lines across 14 crates with 1,767 tests and zero clippy warnings: kernel
(orchestration, workflows, metering, RBAC, scheduler, budget), runtime (agent loop, 3 LLM drivers, 53
tools, WASM sandbox, MCP, A2A), memory (SQLite, vector embeddings, canonical sessions, compaction),
extensions (credential vault, OAuth2 PKCE). One binary, no Docker.
*Read against Air*: the maximal version of the thing Air deliberately is not. Useful as a scale
reference: this is what "build the platform" costs.

**sandbox-agent** (rivet-dev/sandbox-agent) · 1,544★ TypeScript Apache-2.0 · 2026-06-19 · **Watch.**
A static Rust binary that runs *inside* a sandbox and exposes the agent over HTTP, streaming events
and handling permissions, installable with one curl into E2B, Daytona, Modal, Cloudflare Containers
or Docker. The cleanest separation of "where the agent runs" from "who drives it".

**agenttier** (agenttier/agenttier) · 71★ Go Apache-2.0 · 2026-08-24 · **Out of scope.** Kubernetes
pod per agent with default-deny NetworkPolicy, browser PTY, inter-sandbox communication.

**NemoClaw** (NVIDIA/NemoClaw) · 22,262★ TypeScript Apache-2.0 · 2026-08-24 · **Out of scope.** Runs
Hermes, LangChain Deep Agents and OpenClaw inside NVIDIA OpenShell with managed inference, network
policy, snapshots and lifecycle ops. Alpha.

**Open Multi-Agent** (open-multi-agent) · 6,821★ TypeScript MIT · 2026-08-24 · **Out of scope.**
"Describe the goal, not the graph": a coordinator turns a goal into a task DAG, with production
users doing PR review and security analysis.

**agent-runbook** (KnoxOps/agent-runbook) · 17★ Python Apache-2.0 · 2026-07-15 · **Watch.** Compiles
contract-based YAML runbooks into `SKILL.md` plus checkpoint scripts, where every step declares
input and output with JSON Schema. Its framing, "replacing yourself as the one who prompts the
agent", is the same move as Air's launchers.

**skillfold** (byronxlg/skillfold) · 12★ TypeScript MIT · 2026-08-24 · **Watch, small and correct.**
Skills declared in `skillfold.yaml` with exact revisions pinned in `skillfold.lock`, so a clone plus
`skillfold install` gives byte-identical skills. If Air's ported skills ever need to be shared across
repos, this is the mechanism rather than copying directories.

**sub-agents-skills** (shinpr/sub-agents-skills) · 80★ Python MIT · 2026-08-23 · **Watch.** Routes
subtasks to Codex, Claude Code, Grok, GLM, Kimi or Cursor from a portable Markdown skill, with an
explicit warning that isolation guarantees and permission flags are **not equivalent across
backends**. That caveat is the one to remember before assuming a deny list means the same thing
everywhere.

**handoff** (dazuiba/handoff) · 86★ Python · 2026-08-02 · **Watch.** Delegates a task to a cheaper
model from inside a Claude Code or Codex session without switching tools or losing context.

**neuralyzer** (gintasz/neuralyzer) · 39★ TypeScript MIT · 2026-08-07 · **Watch, one tool.** Adds a
single tool the agent can call to wipe its own session context and re-run the first message. The
smallest useful thing in the entire roster, and a good calibration for what "a mechanism" can be.

### A6. Parallel coding agents, desktop and web (53)

The largest category and the most uniform: a window, a worktree per task, a diff view, a merge
button. Notes are short where the project is a variation on that theme. Four are not, and they are
marked Neighbour.

**Zaivern Code** (tacyan/zaivern-code) · 7★ Rust Apache-2.0 · 2026-08-23 · **Neighbour, and the
sharpest single mechanism in the whole sweep.** Agents claim the files, or the individual **line
ranges**, they are about to edit in a shared per-repository **lease ledger**, and **git hooks refuse
a write that would collide**. The README reports the measurement rather than the intention: "with
the lease ledger: 0 / 0, and all 96 edits landed, none refused, 30 of them shifted to a free line
range".
*Read against Air*: a ledger plus a hook that refuses, with a published red/green number, on a
different axis (collision, not verification) from Air's gate. This is the closest thing to Air's
design philosophy found anywhere in the roster, at seven stars, and it is worth reading properly.

**IM.codes** (im4codes/imcodes) · 960★ TypeScript MIT · 2026-08-24 · **Neighbour.** Describes itself
as "not another AI IDE" but "the messaging, memory and review layer around terminal-based coding
agents", with cross-agent audit and **implementation audit and rework gates** where a final scored
verdict of `PASS`, `REWORK` or `BLOCKED` decides whether a run may pass, should repair while limits
allow, or stops. Enrols machines as restricted Controlled Nodes.

**Fletch** (fwdai/fletch) · 20★ Rust AGPL-3.0 · 2026-08-24 · **Neighbour.** Each agent works in an
isolated **clone** (not worktree) inside an OS-level sandbox that denies writes anywhere else, with
a served index of the codebase shared between them, chained into deterministic workflows. States its
position directly: "Nothing merges without you. Live diffs, explicit approval gates, and your review
sit between the agents and the merge... If you want to fire off a dozen agents and merge whatever
comes back, there are simpler tools. Fletch is for the part after the demo: shipping agent-written
code you're willing to own."

**AGX** (ramarlina/agx) · 27★ TypeScript · 2026-05-06 · **Neighbour.** Ticket → implementation → PR →
review loop with humans approving at every gate, state in **SQLite (WAL mode) with durable
checkpoints**. Names the problem the same way Air does: getting agents to do things is no longer the
hard part, keeping track of what they did is.

**agent-orchestrator** (Untrivial-ai) · 9,920★ Go Apache-2.0 · 2026-08-24 · **Commodity+.** Live
kanban over workers, pull requests, CI runs and reviews, with "in review" and "ready to merge" as
first-class columns and an orchestrator that plans and fixes CI failures.

**Orca** (stablyai/orca) · 52,683★ TypeScript MIT · 2026-08-24 · **Commodity, most-starred in the
category.** Fan one prompt across five agents in five worktrees, compare, merge the winner. In-app PR
and board browsing.

**Paseo** (getpaseo/paseo) · 14,907★ · 2026-08-24 · **Commodity.** Daemon plus self-hosted web UI in
Docker, `paseo run --provider ... --worktree feature-x`, phone control.

**superset** (superset-sh/superset) · 13,302★ · 2026-08-24 · **Commodity.** "100+ coding agents in
parallel", worktree each, built-in review.

**t3code** (pingdotgg/t3code) · 20,310★ TypeScript MIT · 2026-08-24 · **Commodity.** Self-described
"agent harness control surface" with a strong mobile app; controls whatever agents are already set
up on your machine.

**Aperant** (AndyMik90/Aperant) · 14,538★ AGPL-3.0 · 2026-06-14 · **Commodity+.** Up to 12 agent
terminals with a self-validating QA loop and automatic conflict resolution on merge back to main; OS
sandbox for bash.

**qm** (yc-software/qm) · 14,152★ MIT · 2026-08-22 · **Out of scope.** Multiplayer harness in Slack
and web, Postgres holding sessions, memory and queue, with identity, policy and scheduler in the API
layer.

**Emdash** (generalaction/emdash) · 5,483★ Apache-2.0 · **Commodity.** Worktree and branch per agent,
diff review, PR creation, CI inspection, merge from one place.

**OpenChamber** · 9,170★ MIT · **Commodity.** OpenCode-centred desktop and web; start a session from
a GitHub issue or PR with context attached, send failed checks back to the agent.

**automaker** (AutoMaker-Org) · 3,216★ · 2026-05-22 · **Commodity.** Kanban-driven feature
implementation in isolated worktrees with a review-and-verify step; Docker isolation from the host
filesystem.

**collaborator** (collabs-inc) · 2,928★ · 2026-08-08 · **Commodity.** Infinite canvas of terminals,
context files and running code instead of tabs.

**CodeNomad** · 2,506★ MIT · **Commodity.** OpenCode as a premium desktop workspace.

**bb** (get-bb/bb) · 2,602★ MIT · **Commodity.** "The agent IDE that builds itself"; worktree starts.

**supacode** · 2,304★ Swift · **Commodity.** Native macOS worktree-per-task command center; every
session exports repo, worktree, tab and surface IDs for scripting.

**coder/mux** · MIT · **Commodity.** Isolated workspaces with a central view of git divergence,
integrated code review, mermaid rendering for agent proposals.

**nimbalyst** · 1,558★ MIT · **Commodity.** Visual workspace, swipe-through diff review, task queue
to keep agents busy.

**synara** · 1,591★ MIT · **Commodity.** Local-first desktop keeping execution and review on the same
task surface; local checkout or managed worktree.

**Traycer** · 1,299★ MIT · **Commodity.** Multi-session workspace with context sharing and agent
messaging.

**Waku** (egoist/waku) · 1,204★ Rust GPL-3.0 · **Commodity.** Native app; a daemon owns task SQLite
data and provider-native sessions; queue or steer follow-ups mid-run.

**jean** (coollabsio/jean) · 1,196★ Apache-2.0 · **Commodity.** Tauri desktop with heavy git worktree
automation (create, archive, restore, delete), GitHub dashboard, checkout PRs as worktrees,
auto-archive on merge.

**Comet** (zeronsh/comet) · 1,111★ Rust MIT · **Commodity.** Per-device engine storing sessions
locally with optional multi-device sync; starts in local-only mode with no account.

**parallel-code** (johannesjo) · 990★ MIT · **Commodity.** "Dispatch in parallel, review the diffs,
merge the wins, toss the rest."

**Berd** (block/berd) · 732★ Apache-2.0 · **Commodity.** Tauri desktop over the Goose backend via
ACP, with the backend pinned by a lockfile.

**kandev** · 685★ Go AGPL-3.0 · **Commodity+.** Customisable workflows, agent profiles, runtimes,
prompts and **review gates**; explicitly "review-first" because "we need to understand and trust the
code that gets deployed".

**Alethe** · 426★ AGPL-3.0 · **Commodity.** Local-first desktop that also manages what agents share:
their CLIs, MCP servers and skills, which drift out of sync in plain terminal tabs.

**Proliferate** · 372★ AGPL-3.0 · **Commodity+.** Worktree per task carrying branch, terminal,
conversation and review state; recurring and event-driven runs (nightly review passes, triage on
alerts).

**dorothy** (Charlie85270/Dorothy) · 339★ MIT · 2026-07-07 · **Commodity.** Kanban with automatic
agent assignment, worktree support, and a "Super Agent" that orchestrates other agents over MCP.

**aizen** · 300★ Swift GPL-3.0 · **Commodity.** Projects, environments and sessions with optional
tmux-backed restore and an in-app MCP marketplace.

**diri** (cristicretu/diri) · 271★ Rust Apache-2.0 · **Commodity+.** Status per session (working /
needs-you / done), tmux-like persistence, worktrees or remote hosts over ssh+tmux, and an MCP server
that lets a running agent spawn another.

**vibe-tree** · 267★ MIT · **Commodity.** One worktree per agent across desktop, web and CLI; every
conversation maps to one reviewable diff.

**constellagent** · 214★ · 2026-05-05 · **Commodity.** Terminal, editor and worktree per agent in one
macOS window.

**clideck** · 152★ MIT · **Commodity.** Browser dashboard; `clideck ask --session "Reviewer"` gives a
scriptable cross-session ask.

**Ouijit** · 153★ AGPL-3.0 · **Commodity+.** Worktree and terminal per task, **five lifecycle hooks**
(start, continue, run, review, done) plus per-project scripts, and sandboxing either in a Lima VM
mounting only that task's worktree or in place under Seatbelt/Landlock.

**Tempest** · 148★ Apache-2.0 · **Watch.** Indexes the codebase once and gives every agent a shared
knowledge base, claiming 86% fewer tokens per agent. The only project here treating shared indexing
as the scaling lever rather than isolation.

**Claude Command Center** · 128★ · **Commodity+.** Answers Claude Code permission prompts from the
dashboard without interrupting a mid-turn session; imports a plan into a queue and drains it with
workers.

**tlbx** · 105★ C# AGPL-3.0 · **Commodity.** Self-hosted browser control station for agents on
Windows, macOS and Linux.

**Garcon** · 62★ · **Commodity.** Self-hosted browser workspace with phone use and Telegram alerts
when work completes, fails, or needs permission.

**Better Agent** · 58★ · **Commodity.** LAN-ready local hub with offline-first capture (queue prompts
while the backend is down) and cross-session search exposed over MCP.

**GraphCode** · 48★ Swift · **Watch.** Agents wired as graph nodes with hand-offs: "where others have
a worktree, GraphCode's is the graph of loops."

**clave** · 47★ MIT · **Commodity.** macOS app for multiple Claude Code sessions.

**vibecraft** · 35★ Apache-2.0 · **Commodity.** RTS-style canvas of agents, folders, terminals and
browsers with worktree actions on folder entities.

**intentic** · 25★ MIT · **Commodity+.** Sandbox per agent on hardware you own, worktree each, "read
every diff before it lands", reopen from any device.

**octomux** · 21★ MIT · **Commodity+.** Kanban fleet view with one permission inbox, per-task choice
of model and harness, and a review workstation that opens with a verdict, a risk read and a ranked
list of things worth looking at, each linked to a line.

**Fletch**, **AGX**, **IM.codes**, **Zaivern Code**: see above.

**agent-squid** · 14★ MIT · **Commodity.** Named lanes, SQLite history and stats, agent-to-agent
invocation syntax.

**ai-maestro** · 757★ MIT · **Commodity.** Born from "I was running 35 agents across terminals and
became the human mailman"; persistent memory, agent-to-agent messaging, multi-machine, tmux.

**humanlayer** · 11,324★ · 2026-06-19 · **Dormant in place.** The repo now says the code is
"pretty much all deprecated" and points at a commercial rebuild. Notable because human-in-the-loop
control was its entire pitch.

**ivy-tendril** · 170★ C# · **Neighbour**, in §3.9.

**takopi** · 1,046★ Python MIT · 2026-05-25 · **Commodity.** Telegram bridge putting agents in chat
threads, branches as worktrees, per-session queue with steering and cancel.

### A7. Personal assistants (28)

Out of scope for Air's question with three exceptions noted below: these are always-on assistants
reachable from chat, not systems that decide whether code work is finished. Included because the
roster's largest projects live here, and because the harness layer the owner asked about is this
layer.

**openclaw** (openclaw/openclaw) · **387,366★** TypeScript · 2026-08-24 · **Out of scope, and the
reason this document exists.** "A personal AI assistant that runs on your devices and meets you in
the channels you already use... designed for a single operator." Installer for macOS, Linux and
Windows plus Docker and Nix paths. Written up in §3.10. Two orders of magnitude more popular than
any coding orchestrator in the roster, and it has no opinion about whether work is done.

**hermes-agent** (NousResearch) · **235,624★** Python MIT · **Out of scope.** Self-improving agent
with a built-in learning loop that creates skills from experience and improves them in use, model
agnostic.

**nanobot** (HKUDS) · 47,349★ Python MIT · **Out of scope.** Ultra-light self-hosted runtime with
WebUI, tools, memory, MCP, cron and subagents.

**QwenPaw** · 34,404★ Python Apache-2.0 · **Out of scope.** Personal assistant for machine or cloud,
multi-chat integration.

**zeroclaw** · 32,645★ Rust Apache-2.0 · **Out of scope.** Fully autonomous assistant infrastructure,
small and fast.

**nanoclaw** (nanocoai) · 30,608★ TypeScript MIT · **Watch (isolation model).** Every agent runs in
its own Linux container seeing only what is explicitly mounted, so bash access is safe because the
command runs in the container, not on the host. Spawns teammates from chat, each with its own bot
identity, container and memory, sharing rooms and canvases. Provisions a Slack app per agent with no
tokens to paste.

**ironclaw** (nearai) · 12,603★ Rust Apache-2.0 · **Watch (security model).** WASM sandbox for
untrusted tools with capability-based permissions, prompt-injection defence (pattern detection,
content sanitisation, policy enforcement), and a Docker sandbox with per-job tokens in an
orchestrator/worker pattern.

**Cloudflare OS** · 9,093★ Apache-2.0 · **Watch.** Workers-based agent workspace where "Gatekeepers"
are supercharged MCP servers mediating all access, and every action an agent performs is logged for
review. Company-internal origin, now open.

**nullclaw** · 8,045★ Zig MIT · **Out of scope.** Assistant infrastructure in Zig.

**lobsterai** (netease-youdao) · 5,943★ MIT · **Out of scope.** Desktop-grade office agent; sensitive
tool actions are permission-gated and logged.

**MetaClaw** · 3,496★ Python MIT · **Out of scope.** Meta-learns from conversations.

**Ouroboros** (razzant) · 1,220★ Python MIT · **Watch, unexpectedly relevant.** A self-modifying
agent whose safety story is process: contributors and coding agents must read `CONTRIBUTING.md`
defining "required project context, verification, and separate-agent review flow"; self-change is
kept inspectable through "git history, review evidence, explicit protected surfaces, and restart
checks"; first-run wizard configures review policy and budget; `ouroboros schedule add --name
nightly-review`. Has an arXiv paper.
*Read against Air*: the only project in the roster that applies an evidence-and-review discipline to
changes to *itself*, which is the same problem Air has when Air edits Air.

**OpenMausBot** · 1,531★ Apache-2.0 · **Out of scope.** Sidebar team of local agents with a VM.

**denchclaw** · 1,646★ MIT · 2026-06-11 · **Dormant/redirected.** README now says migrate to a hosted
product.

**Rakazo** · 1,214★ Apache-2.0 · **Out of scope.** Persistent AI teammates across web, Electron and
Expo; Postgres, Prisma, Graphile; Docker/E2B/Daytona sandboxes.

**rowboat** · 17,393★ Apache-2.0 · **Out of scope.** Indexes your work into a knowledge graph with
surfaces for email, notes, browser and code.

**leon** · 17,456★ MIT · **Out of scope.** Long-running open-source assistant, voice and text.

**picoclaw** (sipeed) · 29,910★ Go MIT · 2026-08-19 · **Out of scope.** Tiny and deployable anywhere.

**zclaw** (tnm) · 2,221★ C MIT · **Out of scope, but a useful yardstick.** A complete personal
assistant in 888 KiB (about 35 KB of app code) running on an ESP32 with GPIO and cron.

**rho** (mikeyobrien) · 369★ MIT · **Watch.** Always-on operator with a **proactive heartbeat**
(check-ins every 30 minutes by default, `rho trigger` to force one), lease heartbeats to keep streams
alive on mobile, line-level code review at `/review`, and outbound policy limits on email.
*Read against Air*: heartbeat-as-attention is the same primitive as Air's channel, applied to a
single agent rather than a fleet.

**Hivekeep** · 48★ MIT · **Watch (packaging).** Self-hosted team of persistent agents in **one
container, one process, one SQLite file**, explicitly "zero Postgres, Redis, Mongo or queue broker",
with composable named allow-lists scoping capabilities per role.

**lemon** (z80dev) · 129★ Elixir MIT · **Watch.** BEAM-native platform with OTP-supervised per-run
processes, SQLite-backed full-text recall, in-process subagent orchestration, MCP client and server.
The only design here that gets crash isolation from the runtime rather than from containers.

**automata** (sentientwave) · 111★ Elixir · 2026-05-05 · **Watch.** Matrix-native workspace whose
stated problem is "it is hard to review what happened after the fact", answered with provider
activity, traces and task history in one admin dashboard.

**assistant** (kcosr) · 90★ · **Out of scope.** Panel-based assistant with plugin panels shared by
Claude, Codex and Pi CLIs over MCP.

**ghostclaw** · 90★ MIT · 2026-05-03 · **Out of scope, cautionary.** A fork with "containers
stripped, full system access, no sandbox, no permission prompts", one process and SQLite state, on a
spare machine. The explicit opposite of every confinement rule Air has.

**lucinate** · 11★ Go Apache-2.0 · **Out of scope.** Terminal chat client for OpenClaw and Hermes
with local skills as slash commands and scheduled routines.

**iva** · Node/SQLite · **Out of scope.** Telegram assistant that builds an Obsidian vault, with cron
and MCP.

**Coworker** (accomplish-ai) · 10,936★ · 2026-08-13 · **Dormant.** "This project is no longer
supported."

### A8. Resting (17)

The roster's own watchlist: no push in months, checked by its author 2026-07-28. Two of these matter
more to Air than most of the active list.

**swarm-protocol** (phuryn) · 53★ TypeScript MIT · last push 2026-03-15 · **Neighbour, dormant.**
"Coordination protocol for agent-first teams. No UI. No sprints. No Jira. Just state sync." Headless
over MCP: claim work, detect file conflicts, heartbeat, hand off across sessions. The smallest
correct statement of Air's coordination layer that exists, and it stopped five months ago.

**gnap** (farol-team) · 81★ MIT · last push 2026-03-17 · **Neighbour, dormant.** Git-Native Agent
Protocol, an RFC draft for coordinating agents through a shared repo as the task board with **zero
servers and no orchestrator process**. Worth reading before Air adds any daemon.

**wit** (amaar-mc) · 46★ MIT · last push 2026-03-27 · **Watch, dormant.** Locks individual
**functions** rather than files using Tree-sitter, declaring intents and warning agents of conflicts
before they write. The finer-grained cousin of Zaivern's line-range leases.

**vibe-kanban** (BloopAI) · 27,904★ Rust Apache-2.0 · last push 2026-04-24 · **Dormant, notable.**
The best-known Rust kanban for coding agents, already covered in
[`prior-art-landscape.md`](prior-art-landscape.md) §B1, and four months without a push.

**subtask** (zippoxer) · 340★ Go MIT · 2026-04-27 · **Dormant.** A Claude Skill that runs tasks
through subagents in git worktrees. Superseded by the harness itself.

**clawe** · 749★ AGPL-3.0 · 2026-02-23 · **Dormant.** "Trello for OpenClaw agents."

**opengoat** · 420★ MIT · 2026-04-12 · **Dormant.** Organisations of OpenClaw agents coordinating
across four coding CLIs.

**antfarm** (snarktank) · 2,495★ MIT · 2026-02-26 · **Dormant.** Build an agent team in OpenClaw with
one command.

**1code** (21st-dev) · 5,607★ Apache-2.0 · 2026-03-06 · **Dormant, archived.** Worktree isolation per
chat, kanban of sessions, MCP plugin marketplace, message queue, review-before-execution.

**CodexMonitor** (Dimillian) · 4,253★ MIT · 2026-03-26 · **Dormant.** Codex session monitor.

**babyagi3** (yoheinakajima) · 129★ MIT · 2026-03-07 · **Dormant.** Minimal agent configured once
then driven by natural language.

**cashclaw** (moltlaunch) · 1,096★ MIT · 2026-03-14 · **Dormant.** Agent that takes work, does work,
gets paid.

**ralphy** · 2,959★ · 2026-02-05 · **Dormant.** Bash Ralph loop across six CLIs.

**wreckit** · 129★ Elixir MIT · 2026-04-14 · **Dormant.** Ralph loop over a roadmap.

**ariana** · 404 on the API, 2026-08-24 · **Gone.** The only roster entry that no longer resolves.

**lettabot** (letta-ai) · 327★ Apache-2.0 · 2026-05-25 · **Archived**, replaced by Letta Code
channels and schedules.

**mercury** (Michaelliv) · 145★ · 2026-03-29 · **Archived.**
