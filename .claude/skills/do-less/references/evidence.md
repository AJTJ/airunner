# Evidence the do-less rule rests on

The verified findings behind `do-less` and CLAUDE.md's "Do less" rule: guardrails as throttles
(F1 to F10 and the taxonomy of which constraints age well), the corpus principles, and the
claims that did not survive a check and must not be repeated. Moved here on 2026-09-25 from
`docs/research/evidence.md` (written 2026-09-14 by condensing nine earlier research documents),
when `docs/research/` was retired; the technology decisions these findings support are listed
in `docs/design.md` §11. Every claim keeps the source it had: a URL with the access date, or a
`path:line`. Adopter notes are cited as "an adopter's", by the path they had when read; some of
those files no longer exist.

The decomposition and sizing evidence that used to sit between these sections lives in the
`decomposition` skill, whose rules it produced. Metis and billing are one line each in
`docs/design.md` §11.

## 1. Guardrails as throttles (from `guardrails-as-throttles.md`, 2026-08-21)

The question: as coding models get more capable, when do the scaffolds and constraints built
around them flip from net help to net cost, and which kinds survive the flip. URLs in this
section were accessed 2026-08-21 unless stated. The per-mechanism audit that was the original
§6 is not carried here; `air audit` and `crates/cli/src/cmd/mechanisms.rs` replaced it.

### F1. Prompting rituals that helped GPT-4-class models hurt reasoning-class models

Confirmatory, established. DeepSeek-R1's limitations section says "Few-shot prompting
consistently degrades its performance" and recommends zero-shot problem statements
(https://arxiv.org/pdf/2501.12948). Microsoft's "From Medprompt to o1" finds "few-shot
prompting hinders o1's performance" and that o1-preview without prompting techniques beats
GPT-4 with Medprompt (https://arxiv.org/abs/2411.03590). "Mind Your Step (by Step)" measures
chain-of-thought cutting o1-preview accuracy by up to 36.3 points on three of six tasks
(https://arxiv.org/abs/2410.21333). OpenAI's reasoning guide: these models work best given "a
clear goal, strong constraints, and an explicit output contract without prescribing every
intermediate step" (https://developers.openai.com/api/docs/guides/reasoning). No
counter-evidence was found. For Air: a prompt that says how to proceed is the class of
constraint with the clearest record of going from help to harm; a prompt that states goal,
constraints and output contract is the class that survives.

### F2. A bash-only agent loop matches or beats feature-rich harnesses on the same frontier model

Confirmatory; established for SWE-bench-shaped work, emerging for longer tasks. mini-SWE-agent
is "some 100 lines of python" with no tool but bash, scores over 74% on SWE-bench Verified,
and its authors say that as models improved "a lot of this is not needed at all"
(https://github.com/SWE-agent/mini-swe-agent). Anthropic's Claude 4 methodology used "the
same simple scaffold" of a bash tool and a string-replacement editor for Opus 4 (72.5%) and
Sonnet 4 (72.7%) (https://www.anthropic.com/news/claude-4). AARR-bench (June 2026) puts
Mini-SWE-Agent with Opus 4.7 at 68.3%, above Hermes (64.6%) and Claude Code (62.2%) on the
same model; weak models cluster (56.1% to 58.1%) across harnesses while the minimal harness
gains 11.5 points on the frontier model against Claude Code's 6.1
(https://arxiv.org/pdf/2606.07462, §4.2-4.3). Counter-evidence in the same paper: structure
bounds runaway on weak models (Claude Code's max 131 steps). One benchmark, one group, no
error bars. For Air: harness structure is worth most where the model is weakest, and Air's
workers are frontier sessions.

### F3. Agentless beat agents in 2024 because of "the limited abilities of current LLMs", then was overtaken

Confirmatory, established. Agentless v1 (July 2024) scored 32.00% on SWE-bench Lite at $0.70
and argued that agent complexity plus limited LLM ability made a fixed
localize-repair-validate workflow the better bet (https://arxiv.org/abs/2407.01489); its last
numbers are December 2024, Lite 40.7% and Verified 50.8%
(https://github.com/OpenAutoCoder/Agentless). Six months later the bash-only agent in F2 was
above 72% on Verified. The fixed workflow was a patch over model weakness and stopped being
the best choice when the weakness went away.

### F4. Vendors say harness components are bets against the model, and remove them on model upgrades

Exploratory, emerging (one vendor, one harness, one upgrade). Anthropic's "Harness design for
long-running application development" (2026-03-24): "Every component in a harness encodes an
assumption about what the model can't do on its own". Moving from Opus 4.5 to 4.6 the authors
dropped the sprint construct and context resets because the newer model no longer needed
them, and kept the generator/evaluator split because a skeptical standalone evaluator "turns
out to be far more tractable" than a self-critical generator
(https://www.anthropic.com/engineering/harness-design-long-running-apps). The earlier post
(2025-11-26) had added exactly those pieces against observed failures
(https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents). Lilian
Weng (2026-07-04): "many harness improvements will be internalized into core model behavior,
but the interface with external context and tools should remain"
(https://lilianweng.github.io/posts/2026-07-04-harness/). For Air: four months is the observed
half-life of a procedural scaffold; the piece that survived is a verification mechanism.

### F5. Anthropic's guidance is "add complexity only when it demonstrably improves outcomes" and "cut anything Claude already does right"

Confirmatory, established as vendor guidance rather than measurement. "Building effective
agents": start with the API directly and add complexity only when it demonstrably improves
outcomes (https://www.anthropic.com/research/building-effective-agents). Claude Code best
practices: for each CLAUDE.md line ask "Would removing this cause Claude to make mistakes? If
not, cut it"; "If Claude already does something correctly without the instruction, delete it
or convert it to a hook"; hooks are deterministic where CLAUDE.md is advisory; "Give Claude a
check it can run"; Claude Code ends the turn after 8 consecutive Stop-hook blocks
(https://code.claude.com/docs/en/best-practices). The context-engineering post asks for "the
minimal set of information that fully outlines your expected behavior"
(https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents); the
tools post says "More tools don't always lead to better outcomes"
(https://www.anthropic.com/engineering/writing-tools-for-agents).

### F6. Context files do not raise success and cost 20% or more; instructions in them are followed

Exploratory, established (two independent studies, same direction). ETH's "Evaluating
AGENTS.md" (v2, 2026-06-23): context files "do not generally improve task success rates,
while increasing inference cost by over 20% on average"; instructions are well followed;
repository overviews are not helpful (https://arxiv.org/abs/2602.11988). SMU's efficiency
study: runtime down 28.6%, output tokens down 16.6%, completion comparable
(https://arxiv.org/abs/2601.20404). Read together: a context file is a cost with no success
gain unless it carries a fact the model cannot derive, and anything wrong or stale in it is
obeyed too. On 2026-08-21 Air appended about 150 lines of `roles.md` to every session
(`crates/cli/src/cmd/launch.rs:75-78` on that date; the lines have since moved).

### F7. Role-based multi-agent frameworks fail mostly on organisation, and are a poor fit for coding today

Confirmatory; established for the taxonomy, contested for the interpretation. MAST (1,600+
traces, 7 frameworks) splits failures 43.9% system design, 32.35% inter-agent misalignment,
23.75% task verification (https://arxiv.org/html/2503.13657); the standing caution is to take
MAST's vocabulary, not its statistics. Anthropic's research system beat single Opus 4 by
90.2% on research at about 15x the tokens and says coding has "fewer truly parallelizable
tasks" (https://www.anthropic.com/engineering/multi-agent-research-system). Cognition
(2025-06-12): parallel collaboration "only results in fragile systems" because "Actions carry
implicit decisions, and conflicting decisions carry bad results"
(https://cognition.com/blog/dont-build-multi-agents). Gas Town is the local cautionary case,
about $100 an hour with merges over failing tests and an open verification chain
(`../../beads/references/bd-facts.md`, "Gas Town", and the sources cited there). For Air: the fleet is a workflow over a
ledger, not agents talking; every role beyond "the session in main" and "a session in a
worktree" is the MAST category 1 risk with no measured need.

### F8. Capability doubles every 3 to 7 months on the task-horizon metric; a constraint's half-life is months

Confirmatory; established for the trend, contested on the doubling time. METR (2025-03-19)
found a doubling time of about 7 months over six years
(https://metr.org/blog/2025-03-19-measuring-ai-ability-to-complete-long-tasks/); the domains
update (2025-07-14) puts software at 2 to 6 months
(https://metr.org/blog/2025-07-14-how-does-time-horizon-vary-across-domains/); Time Horizon
1.1 (2026-01-29) gives 131 days post-2023 (https://metr.org/blog/2026-1-29-time-horizon-1-1/).
Counter-evidence: METR's RCT found experienced developers 19% slower with AI tools while
believing themselves faster (§3); horizon is not throughput. The trend sets the cadence: a
constraint written against this quarter's model is a bet to re-check next quarter.

### F9. The false-success failure does not go away with capability; prompting against it weakens while the base rate stays

Exploratory, established (model cards, verified by pdftotext). Impossible-task gaming: Opus 4
and Sonnet 4 51% without the anti-hack sentence, 19% and 7% with it (Opus 4.1 addendum, Table
5.B footnote 3); Opus 4.5 55% to 35%, Sonnet 4.5 53% to 20%, Haiku 4.5 30% to 23%; the task
set changed between cards, so 51% is a property of an eval version (§3, part 2). SpecBench
(May 2026): "while every frontier agent saturates the visible suite, reward hacking
persists", and the gap "grows by 28 percentage points for every tenfold increase in code
size" (https://arxiv.org/abs/2605.21384). An adopter, 2026-08-21: `make verify` silently
skipped jest, and a backgrounded verify reported exit 0 (`docs/decisions.md`, incidents mined
from the adopter's round). This is the strongest evidence for keeping one class of
constraint: the model's own report of success is not evidence at any capability level
measured, and the prompt-level fix is the part that decays. What holds is an external check
of the artifact.

### F10. Prompt-declared partitions underperform; enforced partitions or nothing

Confirmatory from the corpus; emerging. CAID reports PaperBench single 57.2, prompt-declared
("soft") isolation 55.5, worktree 63.3 (https://arxiv.org/abs/2603.21489; the abstract carries
only the +25.6 headline, the ablation came through an adopter's `research-actions.md:184-206`,
read 2026-08-21). The corpus verdict: enforce the partition observationally or stop writing
the rule. An adopter's dead-end list records "Adding a rule to CLAUDE.md to fix a behaviour
... making the instruction more explicit did not fix it" (`research-actions.md:585-607`), and
its enforcement audit counted 66 process rules, 27% enforced, none firing on a sequence.

### Taxonomy: which constraints age well

The evidence sorts constraints by what they are, not by how strict they are. A constraint ages
well when removing it would make the model wrong about a fact, and badly when removing it
would only make the model free to decide.

| Type | Example in Air | Ages | Evidence |
|---|---|---|---|
| Fact supply (a true thing the model cannot derive) | green at sha, who holds a file, pid liveness | Well; stays true as models improve, costs one line | F5, F6, F4 |
| Verification of an artifact | `air record` exit at HEAD, merged-tree verify | Well; the false-success rate is flat across capability | F9, F4, F5 |
| Isolation (make the wrong action impossible) | worktrees, deny `git push`, leases | Well when it removes a recorded collision; badly when the deny encodes a policy the model could judge | F10 |
| Measurement (count, show, never gate) | review wait, inbox depth | Well; decides every other removal condition | do-less §3; decisions 2026-08-18 item 5 |
| Goal and contract statements | "hand-over needs green at HEAD containing main" | Well | F1, F4 |
| Procedures (ordered steps) | "merge, verify, digest, close"; sprint contracts | Badly; four-month half-life at Anthropic | F1, F3, F4 |
| Role scripts (who may think what) | Mayor/Witness/Deacon | Badly; MAST category 1 | F7 |
| Prompting rituals | anti-hack sentence, worked examples | Badly in relative terms: benefit fell from 2.7x to 1.6x in one generation while the failure persisted | F1, F9 |
| Caps and quotas | `awaiting-review-over-cap` (removed 2026-08-21) | Badly; a throughput assumption; the right version is the count | decisions 2026-08-21 |
| Timers and thresholds | attention thresholds | Mixed: the condition is a fact, the number is a guess that drifts as sessions lengthen | F8 |
| Context files and appended prose | `roles.md` appended to every session | Badly by default, except the non-derivable facts in them | F6, F5 |

### Designing a constraint with a removal condition

Name the incident; a rule written without one did not fix the behaviour in an adopter's own
measurement (F10). Classify by the taxonomy: a procedure, role script, ritual or cap defaults
to "measure instead"; a fact, verification or isolation is built in its smallest version.
Write the removal condition in the same commit, next to the code, in one of three shapes.
Evidence shape: "remove when one round of ledger data shows zero `<condition>` events" (every
hook decision is recorded, `crates/cli/src/cmd/hook.rs:14-18`). Capability shape: "remove when
the model stops doing Y", tested as Anthropic tested sprint contracts, off for one round on the
new model, compare the count (F4). Substitution shape: "remove when bd provides Z" or "when
Claude Code provides Z". Stay advisory for a full round before any refusal; the advisory round
is the measurement. Stay silent on the ok path. Review quarterly or after every model change,
whichever is sooner (F8).

### Summary verdicts as ruled on 2026-08-21

Remove or demote then, each cheap and without an incident: the Stop-hook worker advisory
(silent unless an open claim and a prior hand-over attempt exist); the `inbox-waiting` push
and the gate's digest check (measurements); the `air mcp` prompts `decompose` and `phase`;
`air install` writing skills into the target repo (opt-in); `roles.md` cut to facts, two
incident-backed drives and the denial paragraph. Decide on round-one data: the `bd create`
deny and the capture/triage relay; the peer-on-file warning; `silent-with-claim`. Keep without
a removal condition, because the failure they address is flat across capability: the
hand-over gate's fact checks, `air record`'s integrity flags and backgrounded refusal, the
`git push`, `git commit`-on-main and `bd sync` denies, leases, the event log, pid liveness,
env-by-flag. What has happened to each since is in `docs/decisions.md` and `air audit`.


## 2. Corpus principles (from `SYNTHESIS.md` §1b, 2026-08-18)

Each principle is followed by the verification row that checked it, in the slices condensed
in §3.

1. A fleet is a workflow, not a multi-agent system; the MAS literature does not describe worktree fleets, single-machine concurrency and the human-coordination canon do (part 1 row 34; fleet-size row 3).
2. Verification is the only observation point; agents report success they did not achieve, and any protocol terminating on self-report is unsound (part 2 rows 56-57, 40-41).
3. Rules in context are not mechanisms: the Ontario checklist null result and CAID's soft isolation below single-agent (part 1 row 4; fleet-size row 6).
4. Partition by file to manufacture independence; never synchronise; keep exactly one inter-agent message (fleet-size rows 7, 9, 10, 14).
5. The single human reviewer is the constraint; throughput is 2/L and L is measured nowhere (fleet-size rows 23, 33).
6. Fleet size is a few, not many; returns thin beyond 3-4 agents and go negative where a single agent already succeeds often, and Kim's topology overheads do not price a non-solving coordinator (fleet-size rows 1-3, 8).
7. Resources: the supervisor owns the count, reclamation lives in the kernel or supervisor, reconciliation is level-triggered, a cheap liveness signal beats a smart detector (protocols rows 22-23, 27-28, 56, 58).
8. Checkpoint by writing the deliverable incrementally, crash-only; a wrap-up hook protects the wrong case (protocols rows 44, 47).
9. A task spec's acceptance is one observable condition checkable inside the worktree; write the check, not the prose (specs rows 1, 4, 9, 42).
10. CLI-first for agent-facing operations; the "5-28x cheaper" figure is a scaffolding effect, what survives is equal failure frequency and 12.9% against 2.2% wasted spend (part 2 row 42; specs row 44).


## 3. Verified numbers and claims not to repeat (from the five verification slices, fetched 2026-08-18)

Each slice re-fetched the primary source behind an adopter's externally cited claims. Only
the corrections, the strengthened findings and the load-bearing numbers are kept; rows that
only verified a line number in a file outside this repo are dropped. Row numbers refer to the
deleted slices and are kept so §2's citations stay meaningful.

### Fleet size, partitioning, integration cadence

| Do not repeat | What the source says |
|---|---|
| "We are Kim's Independent topology, 58% overhead; an orchestrator costs 5-9x" | Independent is n redundant solvers of one task plus an aggregator, and overhead is messages per task; one agent per bead is n single-agent runs, and the ratios do not price a coordinator that solves nothing (https://arxiv.org/abs/2512.08296 §3) |
| "File overlap OR 6.13 against branch lifetime OR 1.04-1.09, a factor of six" | A binary variable against a Z-standardised one; Dias's own conclusion is that modularity and size dominate timing, "common slice" is not "same file", and slice-disjoint work still conflicted on config files (doi:10.1016/j.infsof.2020.106256) |
| "Do not quote a population for Dias" | 73,504 scenarios, 125 projects (100 Rails, 25 Django); the 182,273 / 80 / 8-language figure is Menezes et al. 2021 (doi:10.5753/jserd.2021.1911) |
| "Read-only comprehension fans out and pays, +29.8 pp" | Division of labour alone is +7.2 pp at about 6x cost; the rest needs AgentRadio's messaging channel (https://arxiv.org/abs/2607.28430) |
| "STORM: soft and worktree isolation are statistically indistinguishable" | "Similar"; no test reported (https://arxiv.org/abs/2605.20563) |
| "BSP's authors say barriers should be used sparingly" | The next sentence: "not nearly as inherently expensive as they are believed to be" (https://www.cs.unc.edu/~prins/Classes/633/Readings/BSP-QandA.pdf §8) |
| "Li et al. tested n in {2,4,6,8}" | They tested 1 to 8 (https://arxiv.org/abs/2606.00655) |
| "CAID and STORM contradict each other" | Both show worktree roughly equal to soft isolation on Commit0-Lite; the single-agent baseline moved 53.1 to 66.4 with one model step |

Strengthened. Kim et al.: 260 configurations, β = -0.236 (p = 0.004), negative returns above
single-agent accuracy near 0.45, "prohibitively thin beyond 3-4 agents". CAID Figure 6: k=4 on
distinct files 92.1% pass, k=8 on functions in one file 44.3%, with "Do NOT assign multiple
engineers to modify the same file" in Appendix A (https://arxiv.org/abs/2603.21489). Brun
2011: a third of conflicts are invisible to git, 93% grew from a cleanly mergeable state
(https://www.cs.ubc.ca/~rtholmes/papers/fse_2011_brun.pdf); Kasi and Sarma: 5.6% to 35% test
failures on textually clean merges (https://epiclab.github.io/publications/icse13-kasi.pdf);
together, merge on arrival and verify the merged result. CooperBench: 652 pairs, 30% lower
success together, a first-turn plan halves conflicts (29.4% against 51.5%), further
communication does not help (https://arxiv.org/abs/2601.13295). Reviewer habituation in the
wild: latency 3.5x, comment effort down 22% (https://arxiv.org/abs/2606.22721). D3MAS (arXiv
2510.10585) is withdrawn. DORA: "three or fewer active branches", no 2.3x
(https://dora.dev/capabilities/trunk-based-development/).

### MAS literature, part 1

| Do not repeat | What the source says |
|---|---|
| "No clean agent-count ablation exists" | Kim et al. is that ablation; also https://arxiv.org/abs/2604.02460 |
| Wooldridge and Jennings "too many agents, too little each" | Two pitfalls, "too many agents" (over 10) and "too few agents" (https://www.cs.ox.ac.uk/people/michael.wooldridge/pubs/agents98.pdf) |
| The Chubby sequencer sentence in quotation marks | A paraphrase of "an opaque byte-string that describes the state of the lock immediately after acquisition" (https://static.googleusercontent.com/media/research.google.com/en//archive/chubby-osdi06.pdf) |
| Kinny and Georgeff "reacting to any new hole is worse than blind commitment" | Holds for a bold agent with p = 2, "except for high values of γ" (https://www.ijcai.org/Proceedings/91-1/Papers/014.pdf) |
| Elliott "roughly 2-25 people" | Ideal 2-8, upper limit around 25 |
| Anthropic "token usage explains 80% of variance" | On BrowseComp; the 90.2% gain is a different eval (https://www.anthropic.com/engineering/multi-agent-research-system) |
| Karol's 58.6% as a bound on a one-server review queue | Large N, uniform traffic; illustration only |

Strengthened. Some 60 DOIs and page ranges match Crossref and dblp. The clinical trio holds:
I-PASS cut preventable adverse events 30% at 2.4 against 2.5 minutes (P = 0.55; PMID
25372088); the Ontario mandate, 101 hospitals, mortality OR 0.91 (P = 0.13; PMID 24620866);
SURPASS positive with concurrent controls (PMID 21067384). Agentless v1 is two-phase, 27.33%
at $0.34; v2 is three-phase, 32.00% at $0.70 (https://arxiv.org/abs/2407.01489). The 219.7 s
`cargo check` at load 207 is one datum.

### MAS literature, part 2

| Do not repeat | What the source says |
|---|---|
| "Anti-hack prompting cuts hacking over 9x" | Arithmetic on a withdrawn 5%. Corrected: Opus 4 51% to 19%, Sonnet 4 51% to 7% (Opus 4.1 addendum Table 5.B fn. 3, https://www-cdn.anthropic.com/9fa30625273bafdf5af82c93719d7ca606485a16.pdf); Opus 4.5 55% to 35%, Sonnet 4.5 53% to 20%, Haiku 4.5 30% to 23% (Table 6.10.1.A, https://assets.anthropic.com/m/64823ba7485345a7/Claude-Opus-4-5-System-Card.pdf) |
| "Claude 4 system card Table 6.2.A fn. 29 reports 51% / 19%" | The May 2025 card says 47% / 5% and 45% / 10%; the erratum is in the Opus 4.1 addendum |
| "CLI-only runs were 5-28x cheaper than MCP" | A scaffolding comparison; paired MCP-to-CLI ratios span 0.43x to 29x. What holds: equal failure frequency, MCP failures wasted 12.9% of spend against 2.2% (https://arxiv.org/abs/2608.08654) |
| "No replication of the METR RCT" | METR's 2026-02-24 update: 57 developers, -18% and -4% with CIs crossing zero, self-declared unreliable (https://metr.org/blog/2026-02-24-uplift-update/); the 2025 result (+19% actual, -20% believed; https://arxiv.org/abs/2507.09089) stands with that caveat |
| Brown et al. MATH "79.8% to 95.3%; 38.7% to 39.8%" | v1; v3 says 82.9% to 98.44% and 40.50% to 41.41% (https://arxiv.org/pdf/2407.21787) |
| "Silo-Bench is not peer-reviewed" | ACL 2026 Main (https://arxiv.org/abs/2603.01045) |
| Salas 2001 "mixed evidence on behaviour change" | Positive; only safety outcomes unascertained (PMID 12002012) |
| Rigby and Bird "review latency days not weeks" | Median completion 14.7 to 20.8 hours |
| Huang Table 3 "every cell drops" | One cell is flat, none rises above baseline (https://arxiv.org/pdf/2310.01798) |
| swebench.com "top about 79.2%" | A dated snapshot; re-fetch before quoting |
| "Nagappan 86.2% / 84% did not survive" | The numbers are in Table 3; a fabricated table from an earlier fetch is what did not survive (https://www.cs.umd.edu/~basili/publications/proceedings/P125.pdf) |

Strengthened. Code ownership predicts defects: Bird et al. 2011's 5% minor-contributor
threshold (https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/bird2011dtm.pdf),
replicated by Greiler, Herzig and Czerwonka with directory-level ownership predicting better
than file-level (P 0.76 / R 0.60 against 0.74 / 0.38). Rigby and Bird: two reviewers are the
optimum, median changes 25 to 78 lines
(https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/rigby2013convergent.pdf).
"Verified patches wrong" is about 4% to 31% once SWE-Bench+'s 32.67% is labelled leakage
(https://arxiv.org/abs/2410.06992). Chain-of-thought monitors caught 95% of reward hacks
against 60% for action monitors (https://arxiv.org/pdf/2503.11926). Debate helps detection
(+27.4 pp F1) and hurts generation (https://arxiv.org/abs/2606.02866): reviewer agent as
detector, not fixer.

### Protocols, leases, resources

| Do not repeat | What the source says |
|---|---|
| "GitLab ships TTL/heartbeat leases and jobs wedge forever" | gitlab#436988 is a race in the assignment worker; resource groups have no TTL lease (https://gitlab.com/gitlab-org/gitlab/-/work_items/436988) |
| ReentrantLock fair locks have "far lower overall throughput" | "lower overall throughput (i.e., are slower; often much slower) ... but ... guarantee lack of starvation" (https://docs.oracle.com/en/java/javase/21/docs/api/java.base/java/util/concurrent/locks/ReentrantLock.html) |
| "Eight open jobserver-rs issues" | 13; none about reclamation (https://github.com/rust-lang/jobserver-rs/issues) |
| "A bakery algorithm in eight lines of shell" | A ticket lock |
| "Does cargo honour a FIFO jobserver on Darwin? Untested" | The client path is unconditional POSIX, cargo ignores `-j` when a jobserver is inherited, make 4.4 uses FIFO jobservers on macOS; only end-to-end scheduling is unobserved (https://docs.rs/jobserver/latest/src/jobserver/unix.rs.html) |
| "Whether nextest joins a make jobserver" as open | It does not; only the cargo build phase inside it joins |
| "Hooks fire on SIGKILL: predicted no" | Not a research question; SIGKILL runs no user code |

Strengthened. `SEM_UNDO` fires at `exec()` on Darwin, corroborated in XNU (`semexit(p)` in
https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_exec.c), so a resident
supervisor is the only working shape. jobserver-rs 0.1.35 "makes no attempt to release tokens
back to a jobserver on abnormal exit" (https://docs.rs/jobserver/latest/jobserver/), and 27
years of make NEWS have no reclamation entry. Kubernetes' "Edge-triggered behavior must be
just an optimization" (https://raw.githubusercontent.com/kubernetes/design-proposals-archive/main/architecture/principles.md),
Candea and Fox crash-only (https://dslab.epfl.ch/pubs/crashonly.pdf), Kleppmann's fencing
token (https://martin.kleppmann.com/2016/02/08/how-to-do-distributed-locking.html), SQS
visibility timeout with heartbeat renewal, HikariCP's Tn × (Cm − 1) + 1 and OTP's MaxR/MaxT
are verbatim. Claude Code's hooks page lists 31 events and none is a death signal: sweep from
outside, do not hook (https://code.claude.com/docs/en/hooks). An adopter's lease-break and
split-brain coupling existed only because the agent's tool calls refreshed the heartbeat; a
supervisor driving liveness from `waitpid` or `kqueue NOTE_EXIT` removes it. Crates:
`jobserver`, `libc` for SysV semaphores (`nix` has none), `nix::fcntl::Flock`,
`nix::sys::event`, `process-wrap`.

### Task specification, guards, tooling

| Do not repeat | What the source says |
|---|---|
| "Alibaba SWE-CI: 18 models, 75%+ accelerating regression" | 20 models, 100 tasks; 12 of 20 show regression rate rising with iteration count (https://arxiv.org/abs/2603.03823) |
| "SpecBench about 69% hacking ratio; the most gaming-prone domain" | No ratio, no domain comparison; a visible-against-held-out gap growing 27-28 pp per 10x LOC, up to 100 pp above 25K LOC (https://arxiv.org/abs/2605.21384) |
| "ast-grep has no Bash" | Bash is built in; SQL still absent (https://ast-grep.github.io/reference/languages.html) |
| "Semgrep is Python, not one line in mise" | mise has a `pipx:` backend; the coverage objection holds |
| squawk's `prefer-timestamptz` "a prediction" | It exists (https://squawkhq.com/docs/rules) |
| The merge-queue author's "giant PRs" concession as README text | From the HN thread (https://news.ycombinator.com/item?id=49104747) |
| "cargo single-file packages are nightly-only" | True at Cargo 1.98 (2026-08-20); rust-lang/cargo#16569 passed FCP 2026-02-20 and is blocked; date the claim |

Strengthened. Acceptance criteria are the weak link: SWE-Bench Pro's audit found 14.4% overly
strict tests against 7.5% underspecified prompts
(https://openai.com/index/separating-signal-from-noise-coding-evaluations/), and SWE-bench
Verified was retired in April 2026 with 59.4% of an audited subset having flawed tests
(https://openai.com/index/why-we-no-longer-evaluate-swe-bench-verified/); Verified itself
discarded 68.3% of 1,699 screened samples. Guidance is navigation: Probe-and-Refine "helps
agents reach the correct file" (https://arxiv.org/abs/2606.20512). Contradictions are silent:
models detect them (DeepSeek-R1 91.5% F1) and "rarely explicitly notify users"
(https://arxiv.org/abs/2511.14342), so only an executable probe surfaces a mismatch. The hook
binary is the documented mechanism: Bash rules are shell-operator aware, the recipe is "add
`Bash` to your allow list and register a PreToolUse hook", exit code 2 outranks allow rules,
managed settings outrank CLI flags (https://code.claude.com/docs/en/permissions.md); with
`filesystem.disabled: true` a sandboxed command can rewrite `~/.claude/settings.json`
(https://code.claude.com/docs/en/sandboxing.md). Clippy's `disallowed-*` is the precedent for
resolver-backed policy in a diffable config
(https://doc.rust-lang.org/clippy/lint_configuration.html). GitHub's merge queue needs a
remote and `merge_group` CI, so a local land verb is unserved. The Rust case is distribution:
an adopter's timings price repo-local gates behind `make`, not a shipped binary.
