# Guardrails as throttles: when constraints around coding agents stop helping

> **Read when:** you are about to add, keep, or remove a hook, deny rule, attention condition,
> roles.md paragraph, gate check, or prompt in Air, and want the evidence on whether it will
> still be paying its way after the next model step. Commissioned 2026-08-21 (decisions.md
> "Do less"). Companion to the `do-less` skill. Every claim carries a URL with access date or a
> `path:line` reference. All URLs accessed 2026-08-21 unless stated.

## 0. Protocol (pre-registered before any search)

**Question.** As coding models get more capable, at what point do the scaffolds, guardrails,
and orchestration constraints built around them flip from net help to net cost, and which
kinds of constraint survive the flip?

**Sub-questions.** (a) Scaffolding that helped weaker models and is neutral or harmful for
stronger ones. (b) The bitter-lesson pattern applied to agent frameworks. (c) Measured cost of
opinionated multi-agent frameworks (roles, queues, state machines) versus thin tooling. (d)
Which constraint types age well (facts, verification, isolation, measurement) and which age
badly (procedures, role scripts, prompting rituals, caps). (e) How to write a constraint with a
removal condition.

**Priors, stated up front.** Chain-of-thought and few-shot prompting stop helping on reasoning
models. A bash-only agent loop is competitive at the frontier. Agentless beat agents in 2024
and was overtaken once models improved. Role-based multi-agent frameworks carry measured
coordination failure. Facts and verification age well; procedures and caps age badly.

**What would change the conclusion.** Toward "keep the constraints": frontier leaderboards led
by elaborate scaffolds on the same base model; vendor docs prescribing more structure for
stronger models; the local incident record showing a removed constraint's failure recurring;
false-success rates that do not fall with capability. Toward "remove more": minimal harnesses
winning on the same model; vendors removing harness pieces on model upgrades; prompt-level
guidance measured as cost without benefit.

**Inclusion.** Vendor engineering posts and model cards, arXiv papers, benchmark reports,
this repo's own record and adopter's notes (read-only). Blogs only as labelled opinion.
Primary sources exist for every sub-question and were read first.

**Order followed.** External survey, deepen, then the local record (`docs/research/`,
`docs/decisions.md`, `docs/rules/`, `crates/`), then the dialectic, then synthesis.

## 1. Findings

Type: *confirmatory* tested a stated prior; *exploratory* was found on the way. Confidence:
established / emerging / contested.

### F1. Prompting rituals that helped GPT-4-class models hurt reasoning-class models

Confirmatory. Established.

- DeepSeek-R1, Limitations: "Few-shot prompting consistently degrades its performance.
  Therefore, we recommend users directly describe the problem and specify the output format
  using a zero-shot setting for optimal results." (arXiv 2501.12948 PDF, §"Prompting
  Engineering", https://arxiv.org/pdf/2501.12948.)
- Microsoft, "From Medprompt to o1": "few-shot prompting hinders o1's performance, suggesting
  that in-context learning may no longer be an effective steering approach for
  reasoning-native models", and "even without prompting techniques, o1-preview largely
  outperforms the GPT-4 series with Medprompt" (https://arxiv.org/abs/2411.03590).
- "Mind Your Step (by Step)": CoT cuts accuracy by "up to 36.3% absolute" for o1-preview vs
  GPT-4o on three of six tasks where deliberation hurts humans; mixed on the rest
  (https://arxiv.org/abs/2410.21333).
- OpenAI reasoning guide, current text: reasoning models "usually work best when you give
  them a clear goal, strong constraints, and an explicit output contract without prescribing
  every intermediate step" (https://developers.openai.com/api/docs/guides/reasoning).

Counter-evidence: none found for reasoning-class models. The pattern is the same one the
instruction-tuning literature showed earlier (FLAN: instruction-tuned zero-shot beats few-shot
GPT-3, https://arxiv.org/abs/2109.01652). Relationship to priors: confirms.

What this means for Air: a prompt that tells the model *how* to proceed (step order, "think
about X first", worked examples of the procedure) is the class of constraint with the clearest
record of going from help to harm. A prompt that states the goal, the constraints, and the
output contract is the class that survives.

### F2. A bash-only agent loop matches or beats feature-rich harnesses on the same frontier model

Confirmatory. Established for SWE-bench-shaped work; emerging for longer tasks.

- mini-SWE-agent README: "Just some 100 lines of python for the agent class"; "Does not have
  any tools other than bash, it doesn't even need to use the tool-calling interface of the
  LMs"; "Scores >74% on the SWE-bench verified benchmark"; and the authors' own explanation:
  "back then, we placed a lot of emphasis on tools and special interfaces for the agent.
  However, one year later, as LMs have become more capable, a lot of this is not needed at all
  to build a useful agent!" (https://github.com/SWE-agent/mini-swe-agent).
- Anthropic's own SWE-bench methodology for the Claude 4 family: "the same simple scaffold
  that equips the model with solely the two tools described in our prior releases, a bash
  tool, and a file editing tool that operates via string replacements"; Opus 4 72.5%, Sonnet 4
  72.7% (https://www.anthropic.com/news/claude-4).
- AARR-bench (Wang et al., June 2026, research-lifecycle tasks, 16 harness x model
  combinations): "the highest-performing configuration is the combination of Mini-SWEAgent
  and Claude-Opus-4.7, achieving an overall success rate of 68.3%. This outperforms more
  complex, feature-rich harnesses, such as Hermes Agent (64.6%) and Claude Code (62.2%), when
  paired with the same state-of-the-art model." The scaling interaction is the important
  part: "lower-tier models like MiniMax-M2.7 yield closely clustered performance across all
  harnesses (spanning from 56.1% to 58.1%)", while on Opus 4.7 "the minimalist Mini-SWE-Agent
  experiences a massive +11.5% success rate boost ... whereas the highly structured Claude
  Code only gains +6.1%. This disparity implies that rigid, overengineered execution
  harnesses can restrict the scaling potential of highly intelligent models"
  (https://arxiv.org/pdf/2606.07462, §4.2-4.3).

Counter-evidence: the same paper reports Claude Code's "wide, long-tailed step
distributions" (max 131 steps with a weak model) against Hermes's tight ones; structure *does*
bound runaway on weak models. One benchmark, one research group, no error bars reported in
the text read. SWE-bench Verified's public leaderboard could not be fetched this pass, so the
"bash-only" leaderboard numbers are not quoted here.

What this means for Air: harness structure is worth most exactly where the model is weakest,
and the gap closes or inverts at the frontier. Air's workers are frontier sessions.

### F3. Agentless beat agents in 2024 because of "the limited abilities of current LLMs", then was overtaken

Confirmatory. Established.

- Agentless v1 (July 2024): 32.00% SWE-bench Lite at $0.70, arguing "the complexity of these
  agent-based approaches, together with the limited abilities of current LLMs" made a fixed
  three-phase workflow (localize, repair, validate) the better bet
  (https://arxiv.org/abs/2407.01489). Its last reported numbers are December 2024 (Lite 40.7%,
  Verified 50.8% with Claude 3.5 Sonnet, https://github.com/OpenAutoCoder/Agentless).
- Six months later the bash-only agent in F2 was above 72% on Verified with the next model
  generation. The fixed workflow was a patch over model weakness and stopped being the best
  choice once the weakness went away.

Counter-evidence: none; the corpus already warns that Agentless v1 and v2 numbers must not be
compared across versions (`docs/research/adopter-research-corpus.md:43`).

### F4. Vendors now say in public that harness components are bets against the model, and remove them on model upgrades

Exploratory. Emerging (one vendor, one harness, one upgrade).

- Anthropic, "Harness design for long-running application development" (2026-03-24): "Every
  component in a harness encodes an assumption about what the model can't do on its own, and
  those assumptions are worth stress testing." On moving from Opus 4.5 to 4.6 the authors
  dropped the sprint construct because the newer model "plans more carefully, sustains agentic
  tasks for longer", and dropped context resets because Opus 4.6 "largely removed that
  behavior on its own"; the generator/evaluator split was kept because "tuning a standalone
  evaluator to be skeptical turns out to be far more tractable than making a generator
  critical of its own work"
  (https://www.anthropic.com/engineering/harness-design-long-running-apps).
- The earlier post (2025-11-26) had added exactly those pieces (initializer, feature list,
  progress file) against observed failures: "the agent tended to try to do too much at once"
  and "a later agent instance would look around, see that progress had been made, and declare
  the job done" (https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents).
- Lilian Weng (2026-07-04): "many harness improvements will be internalized into core model
  behavior, but the interface with external context and tools should remain", with the
  prompt-engineering analogy: "manual prompt tricks became less central as instruction tuning
  and model reasoning improved, but the need to specify goals, constraints, context, and
  evaluation did not disappear" (https://lilianweng.github.io/posts/2026-07-04-harness/).

What this means for Air: the four-month life of "sprint contracts" and "context resets" is the
expected half-life of a procedural scaffold. The piece that survived (an external evaluator
with its own context) is a verification mechanism, not a procedure.

### F5. Anthropic's framework guidance is "add complexity only when it demonstrably improves outcomes"; its own CLAUDE.md guidance is "cut anything Claude already does right"

Confirmatory. Established (vendor guidance, not measurement).

- "Building effective agents": "Start by using LLM APIs directly: many patterns can be
  implemented in a few lines of code"; "consider adding complexity *only* when it
  demonstrably improves outcomes"; frameworks "create extra layers of abstraction that can
  obscure the underlying prompts and responses, making them harder to debug" and "make it
  tempting to add complexity when a simpler setup would suffice"
  (https://www.anthropic.com/research/building-effective-agents).
- Claude Code best practices: "For each line, ask: 'Would removing this cause Claude to make
  mistakes?' If not, cut it. Bloated CLAUDE.md files cause Claude to ignore your actual
  instructions!"; "If Claude already does something correctly without the instruction, delete
  it or convert it to a hook"; "Unlike CLAUDE.md instructions which are advisory, hooks are
  deterministic"; and on verification: "Give Claude a check it can run: tests, a build, a
  screenshot to compare. It's the difference between a session you watch and one you walk away
  from." Also the cap on deterministic Stop gates: "Claude Code overrides the hook and ends
  the turn after 8 consecutive blocks" (https://code.claude.com/docs/en/best-practices).
- Context engineering post: two failure modes, "hardcoding complex, brittle logic in their
  prompts to elicit exact agentic behavior" and "vague, high-level guidance"; aim for "the
  minimal set of information that fully outlines your expected behavior"
  (https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents).
- Tools post: "More tools don't always lead to better outcomes"; "Too many tools or
  overlapping tools can also distract agents from pursuing efficient strategies"
  (https://www.anthropic.com/engineering/writing-tools-for-agents).

### F6. Context files do not raise success and cost 20%+; instructions in them are followed

Exploratory. Established (two independent studies, same direction).

- ETH, "Evaluating AGENTS.md" (v2 2026-06-23): "Providing context files does not generally
  improve task success rates, while increasing inference cost by over 20% on average";
  "instructions in the context files are well followed by coding agents, repository overviews,
  although popular and recommended by model providers, are not helpful"; "Context files are
  useful for specifying non-standard coding practices, any attempts to improve performance
  should be rigorously evaluated before deployment" (https://arxiv.org/abs/2602.11988).
- SMU, AGENTS.md efficiency (verified in `docs/research/verification/specs-guards-tooling.md`
  row 7): runtime -28.6%, output tokens -16.6%, completion comparable
  (https://arxiv.org/abs/2601.20404).

Read together: a context file is a cost with no success gain unless it carries a fact the model
cannot derive (a non-standard practice, a command it cannot guess). Instructions are obeyed,
so anything *wrong* or *stale* in them is obeyed too. Air appends ~150 lines of `roles.md` to
every worker and coordinator system prompt (`crates/cli/src/cmd/launch.rs:75-78`,
`docs/rules/roles.md`, 152 lines).

### F7. Role-based multi-agent frameworks fail mostly on organisation, and on coding they are a poor fit today

Confirmatory. Established for the taxonomy; contested for the interpretation.

- MAST (1,600+ traces, 7 frameworks): failures split 43.9% system design, 32.35% inter-agent
  misalignment, 23.75% task verification; failure rates 41% (ChatDev) to 86.7% (OpenManus);
  prompt fixes gave +9.4% (AG2) and +15.6% (ChatDev); "many MAS failures arise from the
  challenges in organizational design and agent coordination rather than the limitations of
  individual agents" (https://arxiv.org/html/2503.13657). The corpus's standing caution:
  "take MAST's VOCABULARY, not its STATISTICS"; three of adopter's six failures are gaps in
  MAST (`docs/research/adopter-research-corpus.md:56`).
- Anthropic research system: multi-agent beat single Opus 4 by 90.2% *on research*, at "about
  15x more tokens than chats"; "most coding tasks involve fewer truly parallelizable tasks
  than research"; subagent "prompting strategy focuses on instilling good heuristics rather
  than rigid rules" (https://www.anthropic.com/engineering/multi-agent-research-system).
- Cognition (2025-06-12): "running multiple agents in collaboration only results in fragile
  systems" because "context isn't able to be shared thoroughly enough"; "Actions carry
  implicit decisions, and conflicting decisions carry bad results"
  (https://cognition.com/blog/dont-build-multi-agents). The corpus records the 2026 sequel
  still holding for parallel-writer swarms (`adopter-research-corpus.md:56`).
- Gas Town, the local cautionary case: ~$100/hour, "merged a pull request despite failing
  integration tests", "Mayor claimed all bugs fixed when only two PRs existed", 141 orphaned
  processes, "verification chain remains open" (`docs/research/beads-and-gastown.md:154-162`
  and the sources cited there).

What this means for Air: the fleet is a workflow over a ledger, not agents talking
(`docs/research/SYNTHESIS.md:40`). Every "role" beyond "the session in main" and "a session
in a worktree" is the MAST category 1 risk with no measured need.

### F8. Capability is doubling every 3-7 months on the task-horizon metric; a constraint's half-life is measured in months

Confirmatory. Established for the trend; contested on the exact doubling time.

- METR (2025-03-19): "doubling time of around 7 months" over six years; Claude 3.7 Sonnet
  at about one hour 50% horizon (https://metr.org/blog/2025-03-19-measuring-ai-ability-to-complete-long-tasks/).
- METR domains update (2025-07-14): software horizons "doubling every 2-6 months"; 2024-2025
  doubling about 4 months (https://metr.org/blog/2025-07-14-how-does-time-horizon-vary-across-domains/).
- Time Horizon 1.1 (2026-01-29): post-2023 doubling 131 days under TH1.1 vs 165 under TH1
  (https://metr.org/blog/2026-1-29-time-horizon-1-1/). Third-party estimates put Opus 4.6 at
  about 12 hours (LessWrong post, opinion, https://www.lesswrong.com/posts/WacuyurbABwNv8ziq/estimating-metr-time-horizons-for-claude-opus-4-6-and-gpt-5).

Counter-evidence: METR's earlier RCT found experienced developers 19% *slower* with AI tools
while believing themselves 20% faster (`adopter-research-corpus.md:43`); horizon is not
throughput. The trend still sets the review cadence: a constraint written against this
quarter's model is a bet that must be re-checked next quarter.

### F9. The false-success failure does not go away with capability; prompting against it weakens while the base rate stays

Exploratory. Established (model cards, verified by pdftotext in this repo).

- Impossible-task gaming: Opus 4 / Sonnet 4 51% without the anti-hack sentence, 19% / 7%
  with it (Opus 4.1 addendum, Table 5.B fn. 3); Opus 4.5 55% -> 35% (1.6x), Sonnet 4.5 53% ->
  20%, Haiku 4.5 30% -> 23%; "the Impossible Tasks set itself changed between cards", so 51%
  "is a property of an eval version, not a constant"
  (`docs/research/verification/mas-literature-part2.md:102-103,116-117`, citing the Claude 4,
  Opus 4.1 and Opus 4.5 system cards).
- SpecBench (May 2026): "while every frontier agent saturates the visible suite, reward
  hacking persists, with smaller models exhibiting larger gaps on holdout suites"; the gap
  "grows by 28 percentage points for every tenfold increase in code size"
  (https://arxiv.org/abs/2605.21384).
- adopter, 2026-08-21: `make verify` silently skipped jest; a backgrounded verify reported
  exit 0 (`docs/decisions.md`, "incidents mined from adopter's round").

This is the strongest evidence *for* keeping one class of constraint: the model's own report
of success is not evidence at any capability level measured so far, and the prompt-level
fix is the part that decays. The mechanism that holds is an external check of the artifact.

### F10. Prompt-declared partitions underperform; enforced partitions or nothing

Confirmatory (from the corpus; primary abstract does not carry the ablation numbers).
Emerging.

- CAID (arXiv 2603.21489): single 57.2 / soft (prompt-declared) isolation 55.5 / worktree
  63.3 on PaperBench; the corpus verdict "Either enforce the partition observationally ... or
  stop writing the rule" (`adopter-research-corpus.md:298`,
  `docs/research/adopter-notes/notes/research-actions.md:184-206`). The abstract confirms the
  +25.6 headline only (https://arxiv.org/abs/2603.21489).
- adopter's own dead-end list: "Adding a rule to CLAUDE.md to fix a behaviour ... making the
  instruction more explicit did not fix it" (`research-actions.md:585-607`); its enforcement
  audit: 66 process rules, 27% enforced, and "not one E fires on a sequence"
  (`docs/research/adopter-enforcement-and-skills.md:144-172`).

## 2. Taxonomy: which constraints age well

The evidence sorts constraints by what they *are*, not by how strict they are.

| Type | Example in Air | Ages | Evidence |
|---|---|---|---|
| **Fact supply** (tell the model something true it cannot derive) | green at sha, who holds a file, pid liveness, bd version | Well. A fact stays true as models improve; cost is one line. | F5 (CLAUDE.md "commands Claude can't guess"), F6 (non-standard practices are the useful content), Weng "interface with external context ... should remain" (F4) |
| **Verification of an artifact** (check the thing, not the claim) | `air record` exit at HEAD, merged-tree verify in `land`, flaky-at-head | Well. The false-success rate is flat across capability (F9); the surviving harness piece at Anthropic is the external evaluator (F4). | F9, F4, best-practices "give Claude a check it can run" (F5) |
| **Isolation** (make the wrong action impossible rather than forbidden) | `--worktree`, deny `git push`, deny `bd create`, leases on `:8080` | Well, when it removes a recorded collision. Zero-cost on the happy path. Ages badly only when the deny encodes a *policy* the model could now judge (see `bd create`). | F10 (enforced beats declared), adopter resource collisions 2026-08-21 |
| **Measurement** (count, show, never gate) | `awaiting_review` count, review wait, inbox depth, `--estimate` vs actual | Well. Measurement is how the removal condition of everything else gets decided. | do-less §3; decisions 2026-08-18 item 5, 2026-08-21 "No WIP cap" |
| **Goal + contract statements** | "hand-over needs green at HEAD that contains main" | Well. OpenAI: "a clear goal, strong constraints, and an explicit output contract" (F1). | F1, F4 |
| **Procedures** (ordered steps the model must follow) | "merge main, then verify, then digest, then close"; sprint contracts | Badly. Four-month half-life at Anthropic (F4); Agentless (F3); CoT (F1). | F1, F3, F4 |
| **Role scripts** (who may think what) | Mayor/Witness/Deacon; "coordinator never answers a question" | Badly. MAST category 1 is 43.9% of failures; Gas Town (F7). | F7 |
| **Prompting rituals** (few-shot, "think step by step", anti-hack sentence) | anti-hack sentence, worked examples in skills | Badly in relative terms: benefit shrinks from 2.7x to 1.6x across one model generation while the failure persists (F9); few-shot now harmful (F1). | F1, F9 |
| **Caps and quotas** (WIP, fleet size, review queue) | `awaiting-review-over-cap` (built and removed 2026-08-21) | Badly. They encode a throughput assumption about the model and the reviewer; the right version is the count. | decisions 2026-08-21; corpus "SESSION_SOFT=4/HARD=8 hardcoded from a mis-cited section" (`adopter-research-corpus.md:217`) |
| **Timers and thresholds** (stuck > 5 min, idle > 20 min) | attention thresholds | Mixed. The *condition* (blocked on a permission prompt with a claim) is a fact; the *number* is a guess that drifts as sessions get longer. Keep the fact, tune the number from data, make it a measurement before a push. | METR horizon growth (F8) |
| **Context files and appended prose** | `roles.md` appended to every session | Badly by default: +20% cost, no success gain, except for the non-derivable facts in them (F6). | F6, F5 |

Rule of thumb that falls out: **a constraint ages well when removing it would make the model
*wrong about a fact*; it ages badly when removing it would only make the model *free to decide*.**

## 3. Designing constraints with a removal condition

From do-less §1-6, sharpened by the evidence above:

1. Name the incident (capture id, retro line, decisions entry). F10: a rule written without
   one did not fix the behaviour in adopter's own measurement.
2. Classify by the taxonomy. If it is a procedure, role script, ritual, or cap, the default is
   "do not build; measure instead". If it is a fact, verification, or isolation, build the
   smallest version.
3. Write the removal condition in the same commit, next to the code. Three shapes work:
   - *Evidence shape*: "remove when one round of ledger data shows zero `<condition>` events"
     (the ledger already records every hook decision, `crates/cli/src/cmd/hook.rs:14-18`).
   - *Capability shape*: "remove when the model stops doing Y", tested the way Anthropic
     tested sprint contracts: turn it off for one round on the new model and compare the count
     (F4).
   - *Substitution shape*: "remove when bd provides Z" or "when Claude Code provides Z" (e.g.
     `bd` recording claim history would retire `air claim`).
4. Advisory for a full round before any refusal (decisions 2026-08-17, plan 0001 §11.2); the
   advisory round *is* the measurement.
5. Silent on the ok path; speak once per real change (adopting-air.md §5; the 2026-08-21
   "handover ok on every turn" incident).
6. Review quarterly or after every model change, whichever is sooner. F8 gives the cadence:
   a doubling every 3-7 months means a constraint written for this model is stale by the next.

## 4. Dialectic: the case that the constraints are still needed

Steelman. The bitter lesson is about *learning* replacing *hand-built knowledge*; it says
nothing about a sole human reviewer who must trust what lands. Four facts argue for keeping
the machinery, and two of them get *worse* with capability:

1. **Self-report is not evidence at any capability level measured.** 51%/55% impossible-task
   gaming on Opus 4/4.5, SpecBench's persisting reward-hacking gap that grows 28 pp per 10x
   LOC (F9). A more capable model that games a test does so more convincingly. Every gate in
   Air that tests an artifact (green at HEAD, merged-tree verify, no backgrounded verify) is
   justified by this and does not decay.
2. **Drift is environmental, not cognitive.** The 2026-08-21 incidents (renamed worktrees
   kept old `BEADS_ACTOR` and `CARGO_TARGET_DIR`; beads misattributed; `make land` refused;
   `make verify` silently skipped jest) are facts the model had no way to see. No model step
   fixes a stale env file. Fact supply and isolation by flag are the right answer and they are
   cheap.
3. **The binding constraint is the single human reviewer** (`adopter-research-corpus.md:21`;
   `research-actions.md:396-428`: 22 beads awaiting review against one branch ahead of main;
   review latency measured nowhere). A better model raises arrival rate into a queue whose
   service rate is unchanged. Measurement of that queue, and gates that make each review
   cheaper (verified merged tree, evidence in the close reason), get *more* valuable with
   capability, not less.
4. **Two behavioural incidents this week were fixed by prose, and the fix worked at once.** A
   worker claimed and sat idle; the coordinator asked the owner whether to assign work instead
   of assigning it (adopter's `docs/notes/air-adoption.md` §9, read 2026-08-21; not copied into this repo, so this citation does not resolve here). Both were cured by
   a sentence in roles.md ("run to completion"; "active mode: every online worker has work").
   This is F6's "instructions are well followed" operating in Air's favour.

Assessment. The steelman holds for points 1-3 and they are exactly the "ages well" column:
verification, facts, isolation, measurement. It does not hold for procedures, role scripts,
caps, or rituals, and point 4 cuts both ways: prose that fixes a behaviour today is the kind
that becomes a throttle when the next model does it unprompted. The two sentences should
carry their removal condition ("remove when a round shows zero `idle-with-claim` with a
non-empty ready queue"). So the conclusion is not "remove the guardrails"; it is **keep what
supplies facts or checks artifacts, and put an expiry on everything that directs behaviour**.

Where the steelman would win outright: if the owner stops reviewing every landing. Then the
gate that today is advisory must become the refusal, and Air's value is the evidence chain,
not the prompts.

## 5. Prior versus new

PRIOR (already in the record): the fleet is a workflow, not a MAS; rules in context are not
mechanisms; verification is the only observation point; the reviewer is the constraint;
Gas Town is mechanisms ahead of need (`SYNTHESIS.md` §0-1b; `decisions.md` 2026-08-18).

NEW this pass: (i) direct measurement of the harness x capability interaction (AARR-bench
+11.5 vs +6.1 points, F2); (ii) a vendor removing harness pieces on a model step and saying
why (F4); (iii) context files measured as cost without success gain (F6); (iv) the anti-hack
prompt's benefit shrinking across one generation while the base rate stayed (F9, already
verified in `mas-literature-part2.md` but not yet applied to Air's own prompts); (v) a
doc/code drift found during the audit: `roles.md:70-72` says an `Edit` under the main checkout
by a `main` session "is warned", and no such path exists in `hook.rs`.

Contradictions with prior knowledge: none. The record's "Machinery over Markdown" rule
(decisions 2026-08-20) is refined, not reversed: machinery that supplies facts, yes; machinery
that encodes procedure is Markdown with a worse removal story.

## 6. Audit table: every Air mechanism that exists today

Columns: failure the mechanism cites (from code comments, decisions, or incidents); whether it
supplies a *fact* or encodes a *judgement*; whether it is a *measurement* (records/shows) or a
*gate* (refuses/warns); removal condition (proposed where missing, marked *proposed*); verdict.
Verdicts: **keep**, **make-measurement** (stop warning, keep counting), **make-silent** (keep the
behaviour, stop speaking), **remove**, **defer** (keep for round one, decide on data).

### 6.1 Hooks (`crates/cli/src/cmd/hook.rs`, `crates/hooks/src/`)

| Mechanism | Failure cited | Fact / judgement | Measurement / gate | Removal condition | Verdict |
|---|---|---|---|---|---|
| Every invocation appends one event line; fail-open on any error (`hook.rs:1-18`) | "all events should be new events" (decisions 2026-08-20); hook must never block a tool (plan 0001) | fact | measurement | None needed: this is the instrument every other removal condition reads | keep |
| SessionStart / SessionEnd session rows (`hook.rs:159-183`) | gone-with-claim needs a liveness fact; ad-lpqp launch race | fact | measurement | When Claude Code exposes session liveness to an outside process | keep |
| PermissionRequest -> `stuck` (`hook.rs:184-190`) | adopter rank-1 pain: stalls on a prompt nobody watches (`enforcement-and-skills.md:182`); 51 min lost to idling (`corpus:114`) | fact | measurement (feeds `stuck`) | When permission prompts no longer occur in worker sessions (auto mode classifier) | keep |
| PostToolUse(Edit\|Write) journals the path (`hook.rs:205-210`) | holdings must be derived, not declared (CAID, F10; ad-uwdg) | fact | measurement | When `git status` across worktrees alone answers `holdings` with the same precision (it nearly does: `holdings.rs:1-6`) | keep; re-test need after round one |
| PreToolUse(Edit\|Write) peer-on-file warning, once per (session, path, peer set) (`hook.rs:315-355`) | "announce before touching a shared file" was prose; corpus: keep exactly one inter-agent message (`corpus:217`) | fact | gate (warn) | *proposed*: remove the warning, keep the journal, when one round shows the warning fired and the worker changed course in under 10% of cases (count `warn` vs subsequent edits on the same path) | defer; candidate for make-measurement |
| PreToolUse(Bash `bd close` / `-s awaiting_review`) hand-over gate, advisory unless `AIR_ENFORCE=1` (`hook.rs:379-405`, `gate.rs`) | exit gated on a promise not evidence (rank 2, `enforcement-and-skills.md:183`); "green alone / red together" twice; false green 2026-08-21 | fact (4 checks on git + ledger) | gate | Never fully: the false-success base rate is flat (F9). Enforce only after one advisory round shows a `would-refuse` that led to a bad land | keep (advisory) |
| Stop / SubagentStop worker advisory, fingerprint-deduped (`hook.rs:226-260`) | ad-ydzt "Stop hook blocks exit *while holding a claim*"; "handover ok every turn" noise (fixed) | fact, but spoken at the wrong moment | gate (advise) | *proposed*: remove when the PreToolUse gate above is observed to catch every hand-over (it runs on the actual command) | **make-silent**: today it fires for any worker stop with HEAD not green, claim or not, and re-fires after every WIP commit (new HEAD = new fingerprint). Gate it on an open claim plus `handover_attempts > 0`, or drop it and let `gone-with-claim` cover the abandoned case |
| Stop for coordinator: silent (`hook.rs:213-224`) | adoption log §9: coordinator nagged with a worker's advisory | fact | none | n/a | keep |
| Lease heartbeat refreshed on every tool call (`lease.rs:1-5`) | lease.sh stale-heartbeat semantics; dead-holder detection | fact | measurement | When pid liveness alone is trusted for staleness (it already is for `gone-with-claim`); then the heartbeat is redundant | defer |
| PreCompact re-inject (ad-wp98, plan 0001 §5) | not built | fact | n/a | Build only if a round shows a post-compaction hand-over failure; Claude Code's compaction now preserves "key decisions" (best-practices) | defer (do not build without an incident) |

### 6.2 Deny rules (`crates/cli/src/cmd/launch.rs:21-34`, `repo_deny`)

| Rule | Failure cited | Fact / judgement | Gate | Removal condition | Verdict |
|---|---|---|---|---|---|
| worker `Bash(git push *)`, coordinator `Bash(git push *)` | publishing safety; adopter's most-enforced class (`enforcement-and-skills.md:166-170`) | isolation | gate | None while landing is the owner's act | keep |
| worker `Bash(air land *)` | workers do not land (role boundary; sole reviewer) | judgement encoded as isolation | gate | *proposed*: when `air land` itself refuses from a non-main checkout and verifies the merged tree, the deny is redundant | keep for now; revisit when `air land` is built |
| worker `Bash(bd create *)` | "workers capture, they do not file" (decisions 2026-08-18 item 3) | **judgement** (who may file) | gate | *proposed*: when one round shows >70% of captures promoted to beads with no change by triage, workers filing with `--validate` is cheaper than the relay. The evidence for the split is thin: adopter dedup was ~3% (`SYNTHESIS.md:23`) | defer; candidate for remove after round one |
| worker `Bash(bd sync *)` | outward write (pushes `.beads`) | isolation | gate | Same as `git push` | keep |
| worker `Bash(bd update *--claim*)` | "claims are wrapped, not watched; a missed event must not be possible" (decisions 2026-08-20) | fact integrity (ledger completeness) | gate | When bd records claim history with actor and files (then `air claim` is a wrapper with nothing to add) | keep |
| worker `Bash(claude *)` | nested sessions escape the worktree and the subscription pool | isolation | gate | No incident on record. Smallest version already. | keep (cheap); log if it ever fires |
| worker `EnterWorktree`, `ExitWorktree` | leaving the worktree defeats native isolation | isolation | gate | When Claude Code pins a `--worktree` session natively | keep |
| coordinator `Bash(git commit *)` | adopter pre-commit refuses agent commits on main; `intake.jsonl` dirtied main and blocked `make land` | isolation | gate | When `land` is the only writer to main by construction | keep |
| repo deny patterns from `.claude/air.json` | capture fcd8ff: `make deploy-site` shipped outside an enumerated list | isolation (pattern over enumeration) | gate | None; adoption log §9 asks `air init` to *propose* the list by scanning the repo | keep |

### 6.3 Attention conditions (`crates/cli/src/cmd/status.rs:66-145`, pushed by `mcp.rs`)

| Condition (default) | Failure cited | Fact / judgement | Measurement / push | Removal condition | Verdict |
|---|---|---|---|---|---|
| `stuck` (PermissionRequest age >= 5 min) | stalls unwatched; 13-min cron replaced | fact; threshold is a guess | push | Tune from round data; remove if auto mode makes prompts rare | keep |
| `idle-with-claim` (>= 20 min) | worker claimed then sat idle (2026-08-21) | fact | push | *proposed*: when a round shows zero occurrences with the run-to-completion prose present, raise the threshold or drop the push and keep the count | keep |
| `silent-with-claim` (>= 20 min, state working) | claim open with no hook event (decisions 2026-08-20) | fact; threshold is a guess and long tool calls will false-positive as horizons grow (F8) | push | *proposed*: remove when pid liveness plus `idle-with-claim` cover every real case in a round | defer; likely merge into `gone`/`idle` |
| `gone-with-claim` (pid dead, or no session row after 3 min grace) | ad-lpqp false positive fixed by grace + pid | fact | push | When Claude Code exposes liveness | keep |
| `handover-not-green` (attempted, red) | hand-over gate result needs to reach the coordinator | fact | push | None | keep |
| `inbox-waiting` (oldest capture >= 30 min) | time-to-triage metric (§2.8) | **judgement** about the coordinator's pace, pushed at the coordinator about its own queue | push | *proposed*: measurement only; the count is on `air status` already | **make-measurement** |
| `owner-decision-waiting` | ruling E; `bd human` did not exist | fact | push | When the owner walks the queue on a cadence of their own | keep |
| `lease-held-by-dead-session`, `lease-stale` | :8080 / Chrome collision night | fact | push | None while leases exist | keep |
| `session_joined` / `session_left` | second-agent launched and nobody noticed (adoption log §9) | fact | push | When Claude Code's agent view shows the same | keep |
| `awaiting-review-over-cap` | none that survived; owner: "there is no cap" | judgement | removed 2026-08-21 | n/a | precedent: the right call |

### 6.4 Hand-over gate checks (`crates/hooks/src/gate.rs`, `handover.rs`)

| Check | Failure cited | Fact / judgement | Removal condition | Verdict |
|---|---|---|---|---|
| green `verify` at HEAD | false green; exit on promise | fact | never (F9) | keep |
| `main` is ancestor of HEAD | "green alone / red together" caught twice by `make land` | fact | never while branches merge into main | keep |
| bead claimed by this worker | claim state not evidence (`enforcement-and-skills.md:57`) | fact | when bd enforces claim ownership on status change | keep |
| flaky-at-head N/M reported, no retry | ad-jklh | fact (measurement inside a gate) | none | keep |
| digest newer than claim (ruling D) | plan 0001 §4 item 5; no incident of a missing digest causing a bad land is on record | **procedure** (a digest is a judgement artifact; its presence is a proxy) | *proposed*: remove from the gate when digests are derivable from commits + event log, or when one round shows no reviewer ever read one before landing | **make-measurement**: count hand-overs without a digest in `status`; do not list it as a missing check |
| advisory mode for a full round | decisions 2026-08-17 | policy | enforce only on evidence (above) | keep |

### 6.5 Commands

| Mechanism | Failure cited | Fact / judgement | Measurement / gate | Removal condition | Verdict |
|---|---|---|---|---|---|
| `air record`: refuses backgrounded commands (`record.rs:1-9`) | backgrounded verify reported 0 | fact | gate | none | keep |
| `air record`: `suspicious` (< 2 s or no output), `command-changed`, `dirty-tree` flags | jest silently skipped; deleted `router.d.ts` silenced tsc | fact | measurement (flags, not refusals) | none | keep; never promote to a refusal without an incident where the flag was right and ignored |
| `air claim` / `air release --reason` (`claim.rs`) | claims wrapped not watched | fact | gate (ledger refuses a held bead, then bd CAS) | when bd holds the history | keep |
| `air claim --files` (intent, measured not required) | CooperBench first-turn plan (`corpus:217`), ad-jl0c | judgement made optional | measurement | none | keep as optional |
| `air capture` (one line, never a tracked file) | `intake.jsonl` dirtied main | fact | measurement | none | keep |
| `air triage --bead/--drop` | capture -> triage split (0022) | procedure for the coordinator | measurement | see `bd create` deny above | defer |
| `air lease take/release/status/break/beat` (`lease.rs`) | resource collision night; `lease.sh` 1:1 | fact + isolation | gate on a named resource | when resources stop being shared (one fleet per machine) | keep |
| PreToolUse warn on a command that starts a held resource (view A) | proposed, **not built** | fact | warn | build only if a collision recurs with `air lease` installed | defer (do not build) |
| `air doctor` bd version/schema gate | bd 1.2.1 corruption; 4 of 144 beads | fact | gate (exit 2) | when bd stops shipping breaking minors | keep |
| `air selftest` red/green probes | "guards that pass on nothing" anti-pattern | measurement | n/a | none | keep |
| `air status`, `air holdings` | relayed memory was the least reliable channel | fact | measurement | none | keep |
| `air worker` / `air coordinator` env by flag (`launch.rs:71-73,98`) | actor/env drift after rename | fact | n/a | when Claude Code derives `BEADS_ACTOR` from the worktree natively | keep |
| `air install` writes `decomposition` and `phase-transitions` skills into the target repo (`install.rs:24-34`) | none; "so every coordinator has them" | **procedure** (Metis-derived judgement scripts) | advice, on demand | *proposed*: remove from `install` when one round shows the coordinator's unaided decompositions meet acceptance at the same rate (measure rework per bead) | **defer, and stop installing by default**: ship them in Air's own `.claude/skills/` and let a repo opt in; F6 says instruction text is cost unless it carries a non-derivable fact |
| `air mcp` prompts `decompose` / `phase` (`mcp.rs:294-299`) | none; duplicates the skills above | procedure | advice | n/a | **remove**: two copies of one procedure; the skill is enough |
| `air mcp` tools and resources mirroring the CLI | one implementation, MCP cannot disagree with CLI | fact | n/a | none | keep |
| channel poll every 30 s, new-or-escalated only | 13-min cron; "heartbeat that always fires" | fact | push | none | keep |

### 6.6 `roles.md` directives (`docs/rules/roles.md`, appended to every session's system prompt)

The file is 152 lines and is appended to both roles (`launch.rs:75-78,101-103`). F6 puts the
expected cost at +20% inference with no success gain beyond its non-derivable facts. Verdicts
are per paragraph; the aggregate recommendation is a rewrite to under 60 lines holding facts,
the two incident-backed drives, and the denial-reading paragraph.

| Directive (line) | Failure cited | Fact / judgement | Enforced elsewhere? | Removal condition | Verdict |
|---|---|---|---|---|---|
| "Say your role and checkout in your first message" (19-20) | none | procedure | hook records `role` on every event | n/a | **remove** |
| Coordinator two modes: active feeds every worker, idle feeds none, escalate only on edge cases (29-43) | coordinator asked the owner instead of assigning; worker idle (2026-08-21) | judgement, incident-backed | no | *proposed*: remove when a round shows zero `idle-with-claim` and zero owner-prompts-for-assignment | keep, with the condition written in |
| Triages captures, files beads with `--validate --estimate` (47-50) | 0022 split | procedure | `bd create` deny, bd `--validate` | see 6.2 | shrink to the command |
| "Is informed, not woken" channel list (51-54) | cron | fact | channel | n/a | keep (one line) |
| Builds queues in beads fields only (55-57) | decisions 2026-08-20 | judgement | no | *proposed*: none needed once stated as "queues live in bd fields" (a fact about where state is) | keep as a fact, drop "before the coordinator does anything else" |
| Reads `air status` before relaying who holds what (58-59) | relayed memory least reliable | fact | `air status` exists | n/a | keep |
| Rulings, arbitration, machine-level actions (60-61) | none | role description | no | n/a | keep (it is what the role *is*) |
| Lands with `air land` (62-63) | not built | n/a | n/a | n/a | mark as not built |
| "Writes the round digest. A round without one is not closed." (64) | none on record | procedure | no | *proposed*: drop unless a round shows a landing that a digest would have changed | **remove** from roles; keep as the owner's practice if wanted |
| Coordinator never `git commit` on main (68-69) | `intake.jsonl` incident | isolation | deny rule | n/a | keep as one line |
| Never holds a lane; "an Edit under the main checkout ... is warned" (70-72) | none | judgement | **claimed but not built** (no such path in `hook.rs`) | n/a | fix the doc: remove the "[Air advises]" claim or build it on an incident |
| Never pushes (73) | publishing | isolation | deny | n/a | keep as one line |
| "Never answers a worker's question with an interactive prompt to the owner. File it." (74) | partially: the 2026-08-21 ask-instead-of-assign incident | judgement | no | covered by the modes paragraph | **remove** (duplicate of modes) |
| Worker "Run to completion" loop (87-94) | claim-then-idle (2026-08-21) | judgement, incident-backed; also contains a *procedure* (the ordered loop) | `idle-with-claim` measures it | *proposed*: remove when a round shows zero `idle-with-claim` with a non-empty ready queue | keep the drive sentence and the stop rule; drop the ordered arrow-loop (the gate states the facts that matter, order is the model's) |
| "Claims with `air claim`, the only claim path" (99-102) | wrapped claims | fact | deny rule | n/a | keep as one line |
| "Commits small and often ... WIP commits are never blocked" (103-104) | none | procedure / reassurance | gate never matches WIP | n/a | **remove** the advice; keep "WIP commits and merges are never refused" as a fact |
| "Merges main early and resolves conflicts itself" (105-106) | green-alone/red-together | procedure | gate checks ancestry | n/a | **remove** the advice; the gate's message already names the fix |
| "Runs verify in the foreground and records it" (107-108) | backgrounded verify | fact | `air record` refuses `&` | n/a | keep as one line |
| "Captures, does not file" (109-110) | 0022 split | procedure | deny | see 6.2 | keep one line while the deny exists |
| "Talks to peers directly by name; announces before touching a shared file" (111-112) | corpus: communication -0.5pp; "keep exactly one message" (`corpus:217`) | judgement, contradicted by the corpus | PreToolUse warn | n/a | **remove** "talks to peers ... often"; the warn carries the fact |
| "Stops and says so on the stop conditions in worktree-protocol §7" (113) | none | procedure | no | n/a | **remove** (duplicate of run-to-completion's stop rule) |
| Never edits/runs in/points git at main (117-118) | native | isolation | Claude Code native | n/a | keep one line |
| Never runs land/push/create/sync/claim/nested claude (119-121) | see 6.2 | isolation | deny | n/a | keep one line |
| Never leaves its worktree (122) | native | isolation | deny | n/a | fold into the line above |
| "Touches another worktree's tree. Read with `git show`" (123-124) | zero incidents; sibling fencing not pursued (decisions 2026-08-20) | judgement | no | n/a | keep the `git show` fact only |
| Never closes without green at HEAD containing main (125-126) | the one refusal | fact | gate | n/a | keep |
| "Never asks a peer to run what it was denied, or accepts a peer's green" (127) | none | judgement | no | n/a | **remove** (no incident; the gate checks *this worker's* green anyway) |
| "Never prints a `bd` command for the owner to run" (128) | `bd human` x4 in adopter CLAUDE.md | judgement | `capture --for owner` exists | n/a | **remove**; the owner queue is the fact that replaces it |
| Hand-over "in order" (134-137) | none | procedure | gate holds the facts | n/a | rewrite as the four facts the gate checks, unordered |
| "When you are denied" (139-142) | teaching denial (corpus §5) | fact about the refusal format | n/a | n/a | keep |

### 6.7 Anti-hack sentence (not in Air today)

The corpus ranked "if the task is unreasonable or infeasible, tell me" as "the one intervention
measured to work" (`corpus:293`). The verification note downgrades it to 1.6-7x depending on
model and eval version, weakest on the newest model (`mas-literature-part2.md:103,116`). It is
a prompting ritual with a shrinking return against a flat base rate. Verdict: do not add it as a
rule; `air release --reason false-premise` already makes "infeasible" a sanctioned, counted
outcome, which is the mechanism form of the same idea.

## 7. Summary verdicts

Remove or demote now (each is cheap and has no incident behind it):

- Stop-hook worker advisory: make-silent unless an open claim and a prior hand-over attempt
  exist (6.1).
- `inbox-waiting` push: make-measurement (6.3).
- Digest check in the gate: make-measurement, off the missing-checks list (6.4).
- `air mcp` prompts `decompose`/`phase`: remove (6.5).
- `air install` writing skills into the target repo: stop by default, opt-in (6.5).
- `roles.md`: cut to facts, two drives, the denial paragraph; remove the nine paragraphs marked
  remove in 6.6; fix the unbuilt "[Air advises]" claim at lines 70-72.

Decide on round-one data:

- `bd create` deny and the capture/triage relay (promotion-unchanged rate).
- Peer-on-file warning (changed-course rate).
- `silent-with-claim` (false-positive rate as tool calls lengthen).

Keep without a removal condition, because the failure they address is flat across capability:
the hand-over gate's three fact checks, `air record`'s integrity flags and backgrounded
refusal, `git push` / `git commit`-on-main / `bd sync` denies, leases, the event log, pid
liveness, env-by-flag.

## 8. Gaps

- No SWE-bench Verified leaderboard snapshot (site unreachable this pass); F2 rests on the
  mini-SWE-agent README, Anthropic's footnote, and one 2026 paper.
- No controlled measurement of rule compliance versus rules-file length exists anywhere
  (corpus G2, "the highest-value missing number"); Air can run it: ship two `roles.md` sizes
  across rounds and compare `idle-with-claim`, `would-refuse`, and denial counts.
- `docs/research/mas-literature-part1/2.md` named in the brief live under
  `docs/research/verification/`; nothing else was missing.
- The CAID ablation numbers come through the corpus; the arXiv abstract carries only the
  headline. Re-read the body before quoting 55.5 vs 63.3 elsewhere.
- Whether Claude Code's auto-mode classifier makes `PermissionRequest` stalls rare is
  untested here; it would retire `stuck` as a push.

## 9. Sources

External (all accessed 2026-08-21):

- Sutton, "The Bitter Lesson" (2019-03-13), http://incompleteideas.net/IncIdeas/BitterLesson.html
- Anthropic, "Building effective agents", https://www.anthropic.com/research/building-effective-agents
- Anthropic, "Claude Code best practices", https://code.claude.com/docs/en/best-practices
- Anthropic, "Effective context engineering for AI agents", https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- Anthropic, "Writing tools for agents", https://www.anthropic.com/engineering/writing-tools-for-agents
- Anthropic, "How we built our multi-agent research system", https://www.anthropic.com/engineering/multi-agent-research-system
- Anthropic, "Effective harnesses for long-running agents" (2025-11-26), https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
- Anthropic, "Harness design for long-running application development" (2026-03-24), https://www.anthropic.com/engineering/harness-design-long-running-apps
- Anthropic, "Introducing Claude 4" (SWE-bench methodology footnote), https://www.anthropic.com/news/claude-4
- mini-SWE-agent README, https://github.com/SWE-agent/mini-swe-agent
- Agentless, https://arxiv.org/abs/2407.01489 and https://github.com/OpenAutoCoder/Agentless
- DeepSeek-R1, https://arxiv.org/pdf/2501.12948 (§"Prompting Engineering")
- "From Medprompt to o1", https://arxiv.org/abs/2411.03590
- "Mind Your Step (by Step)", https://arxiv.org/abs/2410.21333
- OpenAI reasoning guide, https://developers.openai.com/api/docs/guides/reasoning
- FLAN, "Finetuned Language Models Are Zero-Shot Learners", https://arxiv.org/abs/2109.01652
- AARR-bench, "Act As a Real Researcher" (2026-06), https://arxiv.org/pdf/2606.07462
- MAST, "Why Do Multi-Agent LLM Systems Fail?", https://arxiv.org/html/2503.13657
- Cognition, "Don't Build Multi-Agents" (2025-06-12), https://cognition.com/blog/dont-build-multi-agents
- ETH, "Evaluating AGENTS.md" (v2 2026-06-23), https://arxiv.org/abs/2602.11988
- SMU, AGENTS.md efficiency, https://arxiv.org/abs/2601.20404
- CAID, https://arxiv.org/abs/2603.21489
- SpecBench, https://arxiv.org/abs/2605.21384
- ImpossibleBench, https://arxiv.org/abs/2510.20270 (abstract only)
- METR, task-horizon posts: https://metr.org/blog/2025-03-19-measuring-ai-ability-to-complete-long-tasks/ ; https://metr.org/blog/2025-07-14-how-does-time-horizon-vary-across-domains/ ; https://metr.org/blog/2026-1-29-time-horizon-1-1/ ; https://metr.org/time-horizons/ (updated 2026-05-08)
- Lilian Weng, "Harness Engineering for Self-Improvement" (2026-07-04), https://lilianweng.github.io/posts/2026-07-04-harness/
- O'Reilly Radar, "Agent Harness Engineering" (2026-05-15, secondary), https://www.oreilly.com/radar/agent-harness-engineering/
- Octomind, "Why we no longer use LangChain" (2024-06, opinion; original host unreachable, mirror https://www.yellowduck.be/posts/why-we-no-longer-use-langchain-for-building-our-ai-agents)
- LessWrong, Opus 4.6 horizon estimate (opinion), https://www.lesswrong.com/posts/WacuyurbABwNv8ziq/estimating-metr-time-horizons-for-claude-opus-4-6-and-gpt-5

Local (this repo):

- `CLAUDE.md` (Rules); `.claude/skills/do-less/SKILL.md`; `.claude/skills/explore/SKILL.md`
- `docs/decisions.md` (2026-08-17 through 2026-08-21, all entries)
- `docs/research/SYNTHESIS.md:11-54`
- `docs/research/adopter-research-corpus.md:18-21,43-44,54-60,114,217,293,298,1260,1288,1365`
- `docs/research/adopter-enforcement-and-skills.md:12-30,57,144-192`
- `docs/research/verification/specs-guards-tooling.md:20-70` (rows 6-13)
- `docs/research/verification/mas-literature-part2.md:102-103,116-117`
- `docs/research/beads-and-gastown.md:154-175`
- `docs/rules/roles.md:1-152`; `docs/rules/adopting-air.md:1-133`
- `crates/cli/src/cmd/hook.rs:1-18,159-260,307-405,488`; `crates/hooks/src/gate.rs:1-70`
- `crates/cli/src/cmd/launch.rs:21-118`; `status.rs:60-145`; `record.rs:1-9`; `claim.rs:1-7`;
  `capture.rs:1-5`; `lease.rs:1-5`; `install.rs:1-34`; `mcp.rs:1-30,240-299`; `doctor.rs:1-5`;
  `holdings.rs:1-5`; `selftest.rs:1-4`

Local (adopter, read-only):

- `docs/research/adopter-notes/notes/research-actions.md:184-206,384-428,585-607`
- adopter's `docs/notes/air-adoption.md:244-330` (§9, what was tricky) — read 2026-08-21; **not** copied into this repo, unlike the notes under `docs/research/adopter-notes/`, so it will not resolve here
