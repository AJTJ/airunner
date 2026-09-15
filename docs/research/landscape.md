# The field: harnesses and orchestrators next to Air

This is the standing answer to one question: is Air a lesser version of something that already
exists, and which parts of Air should be deleted because a harness or a mature project now does
them better. It condenses two documents it replaces:
`docs/research/harness-and-orchestrator-landscape.md` (2026-08-24, 186 rostered projects read one
by one, extended 2026-08-25) and `docs/research/prior-art-landscape.md` (2026-08-17, the
use-or-borrow survey written before Air existed). The data was refreshed on 2026-08-24
(repository metadata from the GitHub REST API, each README read the same day) with a targeted
deep-research pass on 2026-08-25; the next refresh is due 2026-09-24, or the week any Claude Code
release adds an orchestration surface. Condensed 2026-09-14.

## Axes

Nine axes, the things Air claims. A project that scores on the first six is a candidate to
replace part of Air; one that scores on none is a session manager and out of scope however
popular.

1. Durable work store: a task record that survives the session, with dependencies and status.
2. Atomic claim: can two agents take the same item?
3. Recorded evidence: a verification run persisted as a fact (command, exit status, commit).
4. Evidence-gated completion: "yes, but it is told not to" is not a gate.
5. Isolation: worktree, container, VM, or none.
6. Human posture: in the loop, on the loop, or out.
7. Harness-agnostic, or welded to one CLI.
8. Cost to keep: language, runtime, dependencies.
9. License.

## Verdict (2026-08-24)

### Commodity

Worktree per agent, tmux fleets, session dashboards, kanban boards over agent tasks, per-agent
notifications, "which session needs input" inboxes. Roughly 70 of the 186 rostered projects are
exactly this, and Claude Code itself ships worktree isolation, subagents, a workflow runner,
hooks, scheduled runs and background tasks (observed 2026-08-24; inventory in
`docs/research/harness-facts.md` §1). Anything in Air whose job is spawn, isolate,
watch is being commoditised from below and from the side. Assume anything Air does that is not
evidence-keeping will be absorbed within two release cycles.

### Overlaps

The finding that matters. After reading all 186 projects, the count that independently built some
part of what this repo treats as its own is eleven, not the two or three a shallow pass suggested.
URLs are in the roster table; all accessed 2026-08-24.

| Air capability | Already exists as | Shape |
|---|---|---|
| Recorded evidence, gate on completion | loki-mode | Receipts in `.loki/proofs/<run_id>/` split machine facts (command, exit code, gate verdict) from AI judgment; `VERIFIED` needs a real command exiting 0 and a non-empty diff |
| Claim, lease, heartbeat, verify, gate, ledger, in Rust | tutti | `tutti.toml` declares roles and workflows; loop is intake, run, review, gate, record; `tt issue-claim acquire\|heartbeat\|release\|sweep` |
| Coordinator plus worker-per-bead over `bd`, worktree each | orc | Runs `bd init`, decomposes a goal into beads, one engineer per bead per worktree, ephemeral reviewer |
| Ledger plus a hook that refuses a write | Zaivern Code | Line-range leases in a per-repo ledger; git hooks refuse a colliding write; published measurement: 96 edits, 0 refused, 30 shifted |
| Evidence-gated run outcome | MartinLoop | `VERIFIED` / `STOPPED` / `NEEDS REVIEW`; receipts carry verifier evidence and budget posture |
| Verification signals plus tamper-evident history | bernstein | Janitor checks tests, lint, typecheck; HMAC-chained audit log; receipts verified against a public key |
| SQLite work store, atomic claims, handoff notes over MCP | guild | Quests with atomic claims and dependencies, briefs, one Go binary |
| Leases, heartbeats, retries, attempt per worktree | Factory | SQLite state, scheduler with run admission, attempt and result recorded together |
| Work item plus audit trail plus "lands in review, not main" | multica | 47k stars, 23 agent CLIs; intent, run, decisions and diff stay connected |
| Scored completion verdict | IM.codes | `PASS` / `REWORK` / `BLOCKED` decides whether a run may pass |
| Maker/verifier handoff as a field on the work item | 5dive | SQLite queue where every row carries assignee, verifier and handoff position; bash plus systemd |

Plus the ones that record without gating (aGiTrack, codecast) and the ones that gate without
evidence (ORCH, Crewplane, kandev, Fletch, ivy-tendril: review is a person or an agent, not a
recorded fact). The 2026-08-25 deep-research pass (108 agents, 25 sources, 124 claims, 25
adversarially verified, 9 killed) added two off-roster, AgentOps and agentic-os, and confirmed
that the combination below survives. State that survival as "unclaimed among what was reached":
absence of evidence over a bounded set.

### What is still Air's

Three properties, not a component.

- The refusal is on the tracked work item, not on a run the tool orchestrated. loki-mode,
  MartinLoop and bernstein gate a run inside their own loop. Air refuses a `bd close` of a bead
  whatever produced it, because `is_handover_command` matches the close and `AIR_ENFORCE=1` makes
  the refusal real.
- The green must sit at a HEAD that already contains main. No project in the sweep ties evidence
  to a commit that has merged the integration branch. AgentOps comes closest and binds its
  snapshot to filesystem bytes rather than a commit.
- It holds in an interactive session a human is watching. Every neighbour that gates aims at
  unattended work.

Everything else Air does has a better-resourced twin. That is one hook, one SQLite table and one
convention, which is the conclusion `do-less` reaches from the other direction.

The owner asked on 2026-08-24 whether the gate ports. It does. It needs a hook that sees the
completion command before it runs (Claude Code's PreToolUse; OpenCode's `tool.execute.before`,
<https://opencode.ai/docs/plugins/>, accessed 2026-08-24; Codex not checked), a place to read
verify runs, the sha and its merge base, and the id of the work item. Not worktrees, not a
launcher, not a coordinator. Keyed to an id and a sha it survives every harness change.

### Health

Of the 181 rostered projects with resolvable metadata on 2026-08-24: median age 187 days; 47%
created within six months and 90% within a year; 15% with no push in over 90 days and 2% archived;
median contributor count 11, but 29% have three or fewer. Only 26% clear a minimal bar of ten or
more contributors, a push within the last week and six months of history. 90 of 181 are
TypeScript, 20 are Rust. Licenses: 95 MIT, 43 Apache-2.0, 21 unrecognised, 11 AGPL-3.0.

The projects that overlap Air's core are worse than the median on the axis that matters: tutti
has 3 contributors and 27 days without a push; orc 1 contributor and 64 days; Zaivern Code 2
contributors and 35 days of history; MartinLoop 4; 5dive 3; Crewplane 3; loki-mode 6 and a
BUSL-1.1 licence. The two with real teams, multica (272 contributors) and paperclip (194), do not
gate on evidence, so adopting either means keeping Air's hook anyway.

The switching cost is the procedure, not the install: `bd close` refusing without a recorded
green, and a written record of why each mechanism exists. Every neighbour that got close invented
its own nouns for that. Three positions are defensible:

1. Keep Air, delete its commodity half, keep the gate. The recommendation: what Air is uniquely
   good at is also the cheapest to keep. Cost is a monthly hour on this document.
2. Adopt a commodity manager for the fleet (scion for containers, herdr for a terminal fleet) and
   keep Air's hook, ledger and `bd`. Correct once the fleet outgrows what one person can watch or
   isolation must be stronger than a worktree.
3. Switch wholesale to tutti or loki-mode. Not today: both are younger and thinner-staffed than
   what they would replace, and loki-mode's licence is not one to build a workflow on.

Adopting is cheap to try and expensive to reverse; not adopting costs an hour a month of watching.

## Neighbours

Read from primary sources on 2026-08-24 unless noted; URLs in the roster table.

bernstein keeps no LLM in the coordination loop, so a replay reproduces the task graph; a janitor
verifies concrete signals after the agent finishes, and receipts and the audit chain are
checkable with `bernstein audit verify`. The closest thing to Air's gate, from the unattended
direction. MartinLoop is evidence-gated completion named and shipped; its unit is a run, not a
work item.

guild is Air's ledger, claims and handoff surface minus the gate, one Go binary serving SQLite
over MCP, and "switching MCP clients requires no export". If Air's ledger ever needs replacing,
read this first. aGiTrack makes each agent turn a git commit with model and token cost and
surfaces a `merge` command whenever a worktree holds un-integrated work, the signal Air's gate
computes; it records, never refuses. codecast is `air status --attention` and the capture inbox as
a product.

paperclip is the most-starred coding-fleet manager, with heartbeats, atomic ticket checkout and
budget hard stops, and no proof of green before close. Archon encodes the procedure as YAML DAGs
with approval gates; Air deliberately does not encode the procedure (`docs/design.md` §6b;
CLAUDE.md, "This repo's work flow": "the sequence is ours"). kodo verifies by a second agent rather than a recorded
fact. OpenClaw (<https://docs.openclaw.ai/tools/subagents>, accessed 2026-08-24) has no opinion
about whether work is done: a layer Air could sit inside.

AgentOps (accessed 2026-08-25, off-roster) writes BDD acceptance criteria into a bead and judges a
hashed snapshot under `.agents/ao/intents/sha256/`; the second independent adoption of `bd`.
agentic-os (accessed 2026-08-25, off-roster) copies Markdown and scripts into a repo and ships
`validate.sh`, a pre-commit and CI check that fails when a phase's evidence is missing. Strongest
match on mechanism, weakest on scope: the gated unit is its own branch-keyed workflow.

herdr (read first-hand 2026-08-25) is the missing half rather than a competitor: persistent panes,
agent status from foreground processes, a socket API with event subscriptions,
`herdr agent wait <pane> --until done`, `pane.report_agent`, native worktrees since 0.6.2. Checked
across its socket API, changelog and docs: no task store, no verify recording, no gate, no MCP,
no tracker. Its `done` is self-reported, so it can feed an idle signal and never a gate. The
owner declined it on 2026-08-29 (`docs/decisions.md`, "2026-08-29, the post-audit rulings": "our
system works for now").

gastown is the cautionary case in `docs/research/beads.md`, "Gas Town: the Refinery, the
verdict, and what Air copied". swarm-protocol and
gnap, dormant since March 2026, are the smallest correct statements of Air's coordination layer;
read either before adding a daemon.

## The harness layer

Agent = model plus harness, where the harness manages context, tools, permissions, verification
and the loop (attribution to Mitchell Hashimoto, February 2026, is secondary and unverified). The
curated entry point is <https://github.com/ai-boost/awesome-harness-engineering> (accessed
2026-08-24), sections Verification & CI, Human-in-the-Loop, Memory & State, Permissions.

## Evidence

The harness dominates the model. Zhang et al., "Stop Comparing LLM Agents Without Disclosing the
Harness", arXiv 2605.23950 (<https://arxiv.org/abs/2605.23950>, accessed 2026-08-24): harness-induced
variance can exceed model-induced variance, including ranking reversals. No headline number in
the abstract.

What people configure. Galster et al., "Harness Engineering for Agentic AI Coding Tools", arXiv
2602.14690 (<https://arxiv.org/abs/2602.14690>, accessed 2026-08-24), over 2,853 repositories:
context files dominate, AGENTS.md is the emerging cross-tool standard, skills and subagents are
rarely adopted. The field's revealed preference is prose in a context file, which this repo's
"machinery over Markdown" rule says loses. Air bets against the median repository.

Humans on the loop. Fowler, "Humans and Agents in Software Engineering Loops"
(<https://martinfowler.com/articles/exploring-gen-ai/humans-and-agents.html>, accessed 2026-08-24):
maintaining the harness rather than reviewing each output is the posture that scales. Anthropic,
"Measuring AI Agent Autonomy in Practice"
(<https://www.anthropic.com/news/measuring-agent-autonomy>, accessed 2026-08-24): experienced users
move from about 20% auto-approve when new to about 40% past 750 sessions. A challenge to "a human
is always in the loop" as worded; Air's mechanisms (a watchable terminal, `air status`, a gate
that refuses) are on-the-loop mechanisms already, and the prose is stricter than the machinery.

Correction. On 2026-08-24 the assistant told the owner that swapping the harness moves SWE-bench
by 22 points and the model by 1, from <https://amux.io/guides/harness-engineering/>. That figure
did not survive a check and must not be used. Supportable: the variance claim above, and a
reported spread of about 9.5 points holding Claude Opus 4.5 fixed and varying the harness
(<https://www.digitalapplied.com/blog/swe-bench-verified-june-2026-benchmark-vs-scaffolding-analysis>,
accessed 2026-08-24), itself secondary. Direction supported, magnitude not. Unread: arXiv
2606.12344 and arXiv 2606.20683.

## Refresh

Where to look, what to take, how often.

1. The orchestrator roster, <https://github.com/andyrewlee/awesome-agent-orchestrators>. Diff the
   README against the last snapshot (2026-08-24: 186 active, 17 resting); new entries that score on
   axes 1 to 4 get a row in the roster table. Monthly.
2. The harness-engineering roster (URL above), the four sections named. Monthly.
3. arXiv cs.SE and cs.AI for "agent harness", "scaffold", "agentic coding". Monthly.
4. Benchmarks that treat the harness as a variable (SWE-bench Verified, Terminal-Bench,
   Claw-SWE-Bench). Only for the harness-versus-model question. Quarterly.
5. First-party changelogs, highest priority because this is where Air's surface gets eaten: Claude
   Code, Claude Agent SDK, Codex CLI, OpenCode, Gemini CLI. Every release; one that adds
   orchestration, task state or verification forces a same-week re-read of "Commodity".
6. OpenClaw, corellis, ClawTeam. Only if the owner wants the always-on posture. Quarterly.
7. The named neighbours, by releases rather than READMEs: loki-mode, tutti, orc, Zaivern Code,
   bernstein, MartinLoop, guild, Factory, multica, 5dive, aGiTrack, paperclip. Contributor count
   and last push matter as much as features. Monthly.
8. Targeted GitHub search for the residue itself, since a project doing exactly Air's thing is not
   findable by category: `"verify" "close" gate agent commit sha`, `agent "proof of work" tests
   green before close`, `beads OR "issue tracker" agent evidence gate`. Quarterly, and any time
   "What is still Air's" is about to be repeated as a claim.
9. guild, swarm-protocol and gnap as claim-capable alternatives to `bd`. The 2026-08-25 pass
   produced no verified claim about any of them, so Air's most exposed dependency has no measured
   competitor. Answer before the next decision about the claim layer.
10. AgentOps and agentic-os, by release. Commit-binding AgentOps's snapshot is a small step from
    where it is.

Procedure: re-run 1, 2, 5 and 7; update the verdict first and the entries second; log what changed
below; if "What is still Air's" shrinks to nothing, say so loudly, because that retires a
component of Air.

Refresh log:

- 2026-08-25: deep-research pass. herdr re-read first-hand and reclassified from Commodity to the
  fleet layer worth renting; AgentOps and agentic-os added; one error corrected (herdr has managed
  worktrees since 0.6.2).
- 2026-08-24: created from 11 projects, then extended the same day to all 186. The extension moved
  the overlap count from three to eleven and found the four closest projects only in the full
  sweep. The shallow pass was wrong in the direction of flattering this repo, and the projects
  that mattered most had 7, 23 and 112 stars.

## What would change the verdict

- A project that gates a tracked work item, not a run, on recorded evidence in an interactive
  session. Air's remaining reason to exist would be the `bd` integration.
- Claude Code shipping a first-party "task did not verify" refusal, or hooks gaining structured
  verification state. Watch item 5.
- tutti growing past three contributors, or orc past one. Either would be Air's architecture with a
  maintainer base, and adoption would beat re-derivation.
- loki-mode relicensing from BUSL-1.1. Its receipt format is better than Air's.
- guild adding verification, or MartinLoop adding a durable work store.
- The owner deciding the fleet matters more than the gate, in which case adopt scion or herdr for
  the fleet and keep only the hook and the ledger (position 2).

## Considered on 2026-08-17 and not adopted

The prior-art survey ranked these adopt, borrow or optional before Air existed
(`docs/research/prior-art-landscape.md` §H). One line each: what it was, why Air did not take it.

- ACP, the `agent-client-protocol` crate (2.0.0, Apache-2.0): a JSON-RPC session protocol between
  a client and an agent process, ranked adopt as the primary worker interface. Not taken because
  Air launches no agent process to drive: workers are interactive `claude` sessions the owner can
  watch (CLAUDE.md, "A human is always in the loop"), and the gate needs a hook and a work-item id,
  not a wire protocol.
- Symphony's dispatcher `SPEC.md` (openai/symphony): tick loop, claim states, run-attempt state
  machine, capped backoff, ranked adopt as spec. Not taken because Air has no dispatcher: the
  2026-08-17 framing left "runtime vs thin enforcement layer" to research, the first slice
  answered it with a verified-at-sha record and holdings, and the coordinator builds queues in
  beads fields with no Air-side queue (`docs/decisions.md`, entries "2026-08-17, framing" and
  "2026-08-20, first round is for information").
- Vibe Kanban's `executors`, `worktree-manager` and `workspace-manager` crates: ranked fork and
  borrow. Not taken because Claude Code ships worktree isolation natively and Air runs no
  executors; the company shut down 2026-04-10 and the repo has had no push since 2026-04-24.
- pueue and `pueue-lib`: a Rust daemon job queue, ranked optional MVP substrate. Not taken because
  Air has no daemon and a shell exit code carries none of the evidence semantics the gate reads.
- Codex `app-server`: JSON-RPC over stdio for driving Codex, ranked adopt where usage fields are
  needed. Not taken because Air's workers are Claude Code sessions and Codex's hook surface for the
  gate has not been checked.
- herdr: ranked optional substrate for attachable worker sessions on 2026-08-17, re-read 2026-08-25
  and named the adopt candidate for the fleet layer. Declined by the owner on 2026-08-29 with
  scion, tutti and loki-mode (`docs/decisions.md`, "2026-08-29, the post-audit rulings"): position
  1 keeps Air's launcher until the fleet outgrows what one person can watch.
- `air daemon`: `docs/research/evidence.md` (corpus principles) proposed it as layer 3 (dispatcher tick,
  watchdog, merge queue, human queue) for milestone M2 (§5). Not built: the first increment was
  ruled "minimum installable", shipping what existed and running an advisory round before any
  further command (`docs/decisions.md`, "2026-08-20, first round is for information"); the
  coordinator's channel is a ledger poll inside `air mcp` (CLAUDE.md, systems index, `air mcp`
  row); `docs/design.md` §6b carries it on the not-built list; and gnap's zero-server design is
  the reminder that coordination through the shared repo needs no process.

## Roster

Source for every rostered project, including the roughly 150 not listed here:
<https://github.com/andyrewlee/awesome-agent-orchestrators>, snapshot 2026-08-24 (186 active, 17
resting). Stars and language are from the GitHub REST API on 2026-08-24; the overlap column is the
project's own README read that day, so it is the project's claim about itself. This table holds
every entry classed Neighbour (touches Air's residue: evidence, gates, durable work items, claims)
and the Watch entries with a stated bearing on Air. Remaining Watch entries, findable in the
roster by name: ORCH, buzz, Dex, fractal, Loop Engineering, ralph-claude-code, toryo, symphony,
Taskuary, aeon, gh-aw, Agentlas OS, omnigent, openfang, sandbox-agent, agent-runbook, skillfold,
sub-agents-skills, handoff, neuralyzer, agentbox, shire, Tempest, GraphCode, nanoclaw, ironclaw,
Cloudflare OS, Ouroboros, rho, Hivekeep, lemon, automata, wit.

| Project | Stars | Language | What it does that Air also does | Where to look |
|---|---|---|---|---|
| loki-mode | 1,047 | Shell, BUSL-1.1 | Evidence receipts per run separating machine facts from AI judgment; `VERIFIED` needs a real command exiting 0 and a non-empty diff | <https://github.com/asklokesh/loki-mode> |
| tutti | 112 | Rust | Claim, lease, heartbeat, worktree, verify, gate, run ledger; `tt issue-claim`; verify output in `.tutti/state/verify.json` | <https://github.com/nutthouse/tutti> |
| 5dive | 54 | Shell | SQLite task queue whose rows carry assignee, verifier and handoff position; bash plus systemd on 1 GB | <https://github.com/5dive-ai/5dive> |
| orc | 23 | Shell | Coordinator plus one engineer per bead per worktree on `bd`; review is an agent's opinion, no ledger | <https://github.com/spencermarx/orc> |
| gastown | 17,758 | Go | Coordinator over beads, health watchdogs, Bors-style merge queue; see `beads.md`, "Gas Town" | <https://github.com/gastownhall/gastown> |
| paperclip | 79,304 | TypeScript | Heartbeats, atomic ticket checkout, budget hard stops, immutable audit log; no proof of green before close | <https://github.com/paperclipai/paperclip> |
| kodo | 128 | Python | Architect and tester agents review before work is accepted; verification by agent, not by fact | <https://github.com/ikamensh/kodo> |
| bernstein | 976 | Python | Janitor checks tests, lint, typecheck after the agent; HMAC-chained audit log; signed receipts | <https://github.com/sipyourdrink-ltd/bernstein> |
| MartinLoop | 45 | TypeScript | Runs end `VERIFIED`, `STOPPED` or `NEEDS REVIEW` by evidence; budget caps; MCP server | <https://github.com/Keesan12/martin-loop> |
| LoopTroop | 123 | TypeScript | Beads as methodology; QA-fix beads from failed checks; human gates described as becoming optional | <https://github.com/looptroop-ai/LoopTroop> |
| Factory | 202 | Go | SQLite state, leases, heartbeats, retries, one attempt per worktree; no evidence gate | <https://github.com/owainlewis/factory> |
| multica | 47,492 | Go | Work lands in review, not main; intent, run, decisions and diff connected; 272 contributors | <https://github.com/multica-ai/multica> |
| Contrabass | 216 | Go | Symphony port: plan, exec, verify pipeline; JSONL events; file heartbeats | <https://github.com/junhoyeo/contrabass> |
| guild | 313 | Go | SQLite quests with atomic claims and dependencies, handoff briefs, over MCP; no gate | <https://github.com/mathomhaus/guild> |
| Crewplane | 34 | Python | "Make review a gate, not a promise buried in a prompt"; validated workflow nodes in Markdown | <https://github.com/crewplaneai/crewplane> |
| Claudexor | 423 | TypeScript | "Every claim, cost, quota is a typed fact"; unknown cost never recorded as `$0` | <https://github.com/razzant/claudexor> |
| LionClaw | 15 | Rust | Local control plane running agent CLIs as durable, auditable workers with confinement; no gate | <https://github.com/moshthepitt/lionclaw> |
| aGiTrack | 14 | Python | Each agent turn a git commit with model and token cost; detects un-integrated worktree work; records only | <https://github.com/core-aix/agitrack> |
| codecast | 27 | TypeScript | Session triage inbox, line-level attribution, human gate node in workflows | <https://github.com/codecast-sh/codecast> |
| Zaivern Code | 7 | Rust | Line-range lease ledger; git hooks refuse a colliding write; published 96 edits, 0 refused, 30 shifted | <https://github.com/tacyan/zaivern-code> |
| IM.codes | 960 | TypeScript | Scored verdict `PASS` / `REWORK` / `BLOCKED` decides whether a run passes | <https://github.com/im4codes/imcodes> |
| Fletch | 20 | Rust, AGPL-3.0 | Clone per agent in an OS sandbox; "nothing merges without you"; explicit approval gates | <https://github.com/fwdai/fletch> |
| AGX | 27 | TypeScript | Ticket to PR to review with human approval at each gate; SQLite WAL checkpoints | <https://github.com/ramarlina/agx> |
| ivy-tendril | 170 | C#, FSL-1.1 | Plan lifecycle with verification gates and human approval before merge; gates unspecified | <https://github.com/Ivy-Interactive/Ivy-Tendril> |
| swarm-protocol | 53 | TypeScript | Headless coordination over MCP: claim, conflict detection, heartbeat, handoff; dormant since 2026-03-15 | <https://github.com/phuryn/swarm-protocol> |
| gnap | 81 | not recorded | Git-native agent protocol: task board in the shared repo, no orchestrator process; dormant since 2026-03-17 | <https://github.com/farol-team/gnap> |
| AgentOps | 428 | not recorded | Acceptance criteria written into a bead; validate judges a hashed snapshot; ships a `beads` skill (off-roster, 2026-08-25) | <https://github.com/boshu2/agentops> |
| agentic-os | not recorded | Markdown, Bash, Python | `validate.sh` fails a commit when a phase's evidence is missing; branch-keyed workflow, no tracker (off-roster, 2026-08-25) | <https://github.com/KbWen/agentic-os> |
| herdr | 32,266 | Rust | Fleet layer: persistent panes, agent status, socket API with event subscriptions, worktrees; no task store or gate | <https://github.com/herdrdev/herdr> |
| hcom | 461 | Rust | Hooks record activity to SQLite and deliver messages; every agent in a real terminal you can interrupt | <https://github.com/aannoo/hcom> |
| forge-orchestrator | 158 | Rust | Single binary, no daemon: file locking, drift detection over MCP for multi-tool repos | <https://github.com/nxtg-ai/forge-orchestrator> |
| repomon | 16 | Rust | Daemon owns SQLite, watchers and git; the same split as Air's ledger plus channel | <https://github.com/AliHamzaAzam/repomon> |
| tmux-ide | 539 | TypeScript | Adopts an existing tmux session, agent status from Claude Code hooks, fully reversible | <https://github.com/wavyrai/tmux-ide> |
| Archon | 23,266 | TypeScript | YAML DAG of deterministic and AI nodes with approval gates; encodes the procedure, which Air does not | <https://github.com/coleam00/archon> |
