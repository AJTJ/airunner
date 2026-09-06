# Tick 0400 — lane / holding granularity: file, directory, or declared slice?

Date: 2026-08-18 04:00 PDT. Question: for a fleet of ≤4 coding agents in git worktrees on one
repo, at what unit should "holdings" and `air next` overlap be computed — **file**,
**directory/module**, or an **explicit lane label** — to best predict merge conflicts and discarded
work, with the least machinery? Inputs: [fleet-size gap 5](../fleet-size-partition-cadence.md)
("common slice" vs "same file"), [MAS-part2 gap 3](../mas-literature-part2.md) (Greiler directory >
file; Rahman & Devanbu per-file), and the [worker interviews §3](../../worker-interviews-2026-08-17.md)
(workers wanted *file*-level "who is touching which FILE right now"; the conflicts that occurred
were on `log-session.tsx`, `queries.rs`, `profile.rs`, `course-builder.tsx` (rename/modify),
`features.md`, `authorization-matrix.md` — the last two cross-cutting docs).

Method: primary sources fetched 2026-08-18 (PDF text via `pdftotext` where the HTML was
unavailable). Verbatim quotes are marked; everything else is paraphrase.

## 1. Findings

| # | Source | Population | Unit measured | Finding | URL (accessed 2026-08-18) |
|---|---|---|---|---|---|
| 1 | Dias, Borba & Barreto 2020, "Understanding predictive factors for merge conflicts", IST 121:106256 | 73,504 merge scenarios; 100 Rails + 25 Django projects (2007–2017, GitHub, >500 stars) | **MVC slice** = "a group of related model, view, and controller files that can be traced and matched by combining both the naming conventions and standard directory structure" (§3.2). Binary: "1 when both contributions changed at least one file related to the same slice". No same-file variable was fitted; only slice, files-changed count, commits, developers, lines, timing. | Common slice OR 6.13 univariate, 3.78–5.18 multivariate (Table 4). **But** RQ1: "merge conflicts also occur with modular merge scenarios, which change disjoint sets of slices (42.7%). So aligning slice and task structure … gives no guarantees of conflict avoidance." Manual inspection: "conflicts are caused because of parallel changes to files that are not part of the slice structure; this includes configuration files, and files that define classes reused across slices" — `config/deploy.rb`, `Gemfile.lock` (0–100% of a project's conflicts), shared JS. Coarser MVC-module grouping (Model/View/Controller/Config/App) fails: "most contributions (65.3%) affect more than one MVC module"; "aligning task structure with the MVC module structure … is not supported by our sample". Tooling advice: awareness tools should warn "when they change the same application slice, instead of just warning when they change the same file". | https://doi.org/10.1016/j.infsof.2020.106256 ; PDF https://pauloborba.cin.ufpe.br/publication/2020understanding_predictive_factors_for_merge_conflicts/2020ISTPredictiveFactorsForMergeConflicts.pdf |
| 2 | Bird, Nagappan, Murphy, Gall & Devanbu 2011, "Don't Touch My Code!" (FSE) | Windows Vista and Windows 7 | **Binary/component** (not file) | "measures of ownership such as the number of low-expertise developers, and the proportion of ownership for the top owner have a relationship with both pre-release faults and post-release failures" (abstract). Minor contributors are the defect signal. Unit is far coarser than a file. | https://www.microsoft.com/en-us/research/publication/dont-touch-my-code-examining-the-effects-of-ownership-on-software-quality/ |
| 3 | Rahman & Devanbu 2011, "Ownership, experience and defects: a fine-grained study of authorship" (ICSE) | Multiple OSS projects (line-level via git blame on bug-fixed fragments) | **Code fragment / file** | "implicated code is more strongly associated with a single developer's contribution; … an author's specialized experience in the target file is more important than general experience" (abstract). The signal is *per-file* expertise, not headcount. | https://doi.org/10.1145/1985793.1985860 (Semantic Scholar record) |
| 4 | Greiler, Herzig & Czerwonka 2015, "Code Ownership and Software Quality: A Replication Study" (MSR) | 4 major Microsoft products | **File vs directory** ownership metrics, classifying bug-containing units | File level: "median precision of 0.74 and a median recall of 0.38"; directory level: "precision of 0.76 and a recall of 0.60". Ownership correlates with quality; directory aggregation mainly raises **recall** (fewer missed defective units) at equal precision. This is a *defect* outcome, not a merge-conflict outcome. Foucault et al. 2015 (OSS) failed to replicate. | https://www.microsoft.com/en-us/research/publication/code-ownership-and-software-quality-a-replication-study/ |
| 5 | CAID — Geng & Neubig 2026, "Effective Strategies for Asynchronous Software Engineering Agents", arXiv 2603.21489 v2 | PaperBench (k=2), Commit0-Lite (k=4), Sonnet 4.5 et al.; simpy N=4 vs N=8 case study | **File** (manager partitions "at the file level first"; function level only inside one file); a **restricted shared-file class** | Manager prompt: "Ensure that each engineer creates and modifies only their own files. Do NOT assign multiple engineers to modify the same file, as this will cause merge conflicts." §3: "Certain shared files, such as package initialization files (e.g., `__init__.py`), are marked as restricted, and engineers are explicitly instructed not to commit changes to them." simpy: N=4 "no two engineers work on the same file at the same time … 92.1%"; N=8 "different functions within the same file (notably events.py) … 44.3%"; "a delegation that ignores the ownership boundaries of the file-level." | https://arxiv.org/abs/2603.21489 (PDF text) |
| 6 | STORM — Liu et al. 2026, "Multi-agent Collaboration with State Management", arXiv 2605.20563 | Commit0-Lite, PaperBench; Sonnet 4.6; shared workspace (no worktrees) | **File** version counters + read-snapshot ("local state consistency"); short per-file **reservation** after a rejected write; intent annotations for semantic (sub-file) conflicts | Contribution 1: "multi-agent state management as file-level context consistency: an agent's write is valid only if the target file and its read dependencies have not been modified since the agent last observed them." "in a well-decomposed task, most concurrent edits touch different files"; "Version tracking catches file-level conflicts but not semantic ones." Delegation prompt still says "split the major tasks at file level first"; engineer prompt: "DO NOT modify files that belong to other engineers." Related-work: "optimistic concurrency control improves over lock-based shared-state coordination". | https://arxiv.org/abs/2605.20563 (PDF text) |
| 7 | AgenticFlict — Ogenrwot & Businge 2026, arXiv 2604.03551 v2 | 142K+ agent PRs / 59K+ repos; 29,609 conflicting PRs (27.67%); 336,380 conflict regions | **PR → conflicting file → conflict region** (detected by `git merge --no-commit --no-ff` and marker parsing) | Mean 4.36 / median 2 conflicting files per conflicting PR; 11.36 regions per PR. **No breakdown by file type/directory is published** although the schema has a file-extension field — the "which files conflict" question is open on agent PRs. | https://arxiv.org/abs/2604.03551 ; https://arxiv.org/html/2604.03551v2 |
| 8 | Claim Plane — Nikolaev 2026, arXiv 2607.21909 | Design paper; 6 CooperBench pairs "only as feasibility evidence … intentionally too small for comparative claims" | **Declared typed resources; same-file parallelism constrained to declared regions**; contingent scope promoted on first mutation | Pre-write admission with declared ChangeIntents; "constrains same-file parallelism to declared regions, serializes unresolved overlap … fails closed on ambiguous authority"; "a contingent mutation does not reserve write ownership initially; the first attempted mutation triggers atomic scope promotion". Static intents forced full serialization (6/6 pass); dynamic scope kept parallelism on half. Evidence that *declared-up-front* file sets are either over-broad (serialize everything) or need dynamic promotion — i.e., derive from actual edits. | https://arxiv.org/abs/2607.21909 |
| 9 | Overstory (jayminwest/overstory), README | OSS orchestrator, 2026 | Worktree per agent; FIFO merge queue "with 4-tier conflict resolution"; a "soft FILE_SCOPE violation detection" for builders/mergers; role-based tool-call guards | **No file-claim / lock registry**; per-agent *declared file scope* is checked softly after the fact; conflicts handled at merge time at git's file granularity. | https://github.com/jayminwest/overstory |
| 10 | Gas Town (steveyegge/gastown) | OSS orchestrator, 2026 | Worktree per polecat under `.gc/worktrees/<rig>/polecats/<name>/`; Refinery merges sequentially with rebase | **No file locking**; isolation + serial merge queue; work claimed at the bead/issue level, not file level. | https://yegge.ai/gastown ; https://github.com/steveyegge/gastown |
| 11 | Symphony (openai/symphony) | SPEC/WORKFLOW.md (re-fetched in [tick 0230](2026-08-18-0230-what-to-work-on.md)) | Worktree per issue run; claims at issue level; eligibility + sort by priority/age | **No file or directory claims**; overlap not modelled at all. | https://raw.githubusercontent.com/openai/symphony/main/elixir/WORKFLOW.md (via tick 0230) |
| 12 | Worker interviews (the adopter's round 2026-08-17) | 4 workers + coordinator, one round | Declared lanes were **directories** ("I hold backend/src/events/*"); conflicts were **files** | Every reported conflict was on a specific file the lane did not predict: `log-session.tsx` (three agents), `profile.rs` (two), `course-builder.tsx` rename/modify, `queries.rs` hunk staling 8 `authorization-matrix.md` citations, `features.md` rows, `Makefile`/`.gitattributes` unmentioned. Workers asked for "who is touching which FILE right now"; `make fleet` "can't see uncommitted edits". | [worker-interviews-2026-08-17.md](../../worker-interviews-2026-08-17.md) §2–§4 |

## 2. Synthesis

1. **Nobody has fitted "same file" against "same directory" as merge-conflict predictors on the
   same data.** Dias fitted *slice* only (a naming-convention grouping that spans directories);
   Greiler's directory > file result is about *defect* recall, not conflicts; CAID/STORM *assume*
   file ownership in the prompt and report that violating it hurts (92.1 → 44.3). AgenticFlict has
   the population to answer it and did not. So the choice below is engineering judgment on
   converging but non-identical evidence, not a measured effect size.
2. **File is the unit every agent-era system actually enforces or detects on** (CAID prompt and
   `__init__.py` restriction, STORM version counters, Overstory FILE_SCOPE, Claim Plane's
   admission — the only sub-file design — is a preprint with n=6). Git itself reports conflicts per
   file. The interviews' failure reports are per file. File is also the unit with the least
   machinery: it falls out of `git status`/`git diff <merge-base>` and the `PostToolUse` path.
3. **Coarser units mis-predict in both directions.** Directory lanes under-predict (all four
   The adopter conflicts were files outside or across declared directories; Dias: 42.7% of conflicts
   were slice-disjoint) and over-predict (Dias: 65.3% of contributions touch >1 MVC module, so a
   module lane would flag most pairs). Greiler's gain from directories is *recall*, which for a
   warn-only signal to an agent means more warnings — the interviews already put the
   false-positive tax of guards at ~15 refusals/session. Directory aggregation therefore belongs
   at **query time as a second-rank tie-breaker**, not as the stored unit.
4. **The cross-cutting class is real and separate.** Dias (config, `Gemfile.lock`, shared
   classes), CAID (`__init__.py` restricted), the adopter (`features.md`, `authorization-matrix.md`,
   `Makefile`, `.gitattributes`, `openapi`, `CHANGELOG`) all show conflicts concentrated in files
   *no lane owns*. No slice/lane label predicts them; a **short committed list** does. These files
   should be flagged whenever *two* live workers hold them, regardless of ranking, and land order
   should be advised on them.
5. **An explicit lane label adds little predictive value beyond derived paths.** Its two remaining
   uses are (a) filtering `next` *before* any edit exists (the derived set is empty at claim time)
   and (b) the coordinator's partition intent (Dias: "avoid the parallel execution of tasks that
   focus on common slices"). Claim Plane's own result — static declarations serialize everything;
   dynamic promotion is needed — argues that a declared lane must be treated as a *hint* that the
   journal overrides, never as authority. Rahman & Devanbu's per-file specialisation adds a soft
   ranking cue (prefer the worker who last edited the file), free from git log.

## 3. Recommendation for plan 0001 (check 6 and `air next`)

**Unit of record: file path.** `edit_journal` and holdings store paths only (already so). Do not
store directory or lane rows; derive `dirname(path)` at query time.

**`air next` ranking (in order):**
1. **Cross-cutting file held by a live peer** — a committed list in `.air/config.toml`
   (`shared_files = [...]`, seeded from the adopter's own history: `features.md`,
   `authorization-matrix.md`, `openapi.*`, lockfiles, `Makefile`, `CHANGELOG`). Not a rank —
   an always-printed flag with the peer and sha, plus "merge <peer>@<sha> first" advice.
2. **Same-file overlap** between the bead's cited/likely files and live holdings (uncommitted +
   committed-since-merge-base). Primary rank; print *which peer, which file*.
3. **Same-directory overlap** (parent dir of cited files vs parent dirs of holdings) —
   tie-breaker only; never a warning by itself.
4. **Freshness cue** (optional, M1): worker who most recently touched the cited files
   (`git log -1 --format=%an -- <file>`) — Rahman & Devanbu specialisation; measure before
   promoting it.

**Check 6 (lane at claim):** keep the *optional* declared file/dir list on `claims` as a
**hint** that seeds ranking before the first edit; the journal supersedes it once edits exist.
Do **not** refuse a second claim on directory overlap; warn on same-file overlap only (plan §5
`PreToolUse` already does this per file). Sub-file (function/region) claims: not built.

**Do NOT build:** a lane taxonomy or slice detector (Dias's slices are Rails/Django naming
conventions; the repo has none); a lock/claim registry (STORM: OCC beats locks; Overstory, Gas Town,
Symphony all ship without one); directory-level rows in the ledger; a Claim Plane–style admission
control; conflict *prediction* by branch age or size (Dias: duration "not as relevant").

**Measure (plan §8):** per landed bead, whether the eventual conflict/citation-staling file was (a)
in the shared list, (b) same-file-flagged, (c) only directory-flagged, (d) unflagged. If (c) is
non-trivial after two rounds, promote directory overlap to a warning; if (d) dominates, revisit.

## 4. Edits made

- `docs/plans/0001-first-slice.md` §2 row 2 (paths only; directories derived) and §3 `air next`
  (rank order + cross-cutting class), each citing this tick.
- README line appended.
