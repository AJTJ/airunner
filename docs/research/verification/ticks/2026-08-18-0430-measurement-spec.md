# Tick 0430 — Measurement spec: review latency, success rate, discarded work (2026-08-18)

**Question.** Which ledger facts and derived metrics make the numbers the corpus says are
"measured nowhere" fall out of normal operation of Air, with definitions precise enough to
implement once and compare across fleet rounds?

**Inputs read.** Plan 0001 §2 (tables 1–6) and §8; SYNTHESIS §1b (2/L; "single-agent success
rate per bead; review latency" as open questions to *measure*); [fleet-size slice gaps 2 and 4](../fleet-size-partition-cadence.md)
(solo-baseline drift after each model step; L and reviewer habituation, 2606.22721 as the
instrument); [MAS part 1 gap 2](../mas-literature-part1.md) (M/M/1/K has no λ or μ);
[MAS part 2 gap 4](../mas-literature-part2.md) (METR 2026: timing unmeasurable when several agents
run; log claim-to-merge wall-clock instead); [coordinator interview §6(i)](../../coordinator-interview-2026-08-17.md)
("nothing reached main for 2 h … review latency is unmeasurable"; single lander);
`adopter-notes/notes/research-actions.md` — success rate ≠ duration (`:337-343`, `:578-583`),
"review latency is measured nowhere" (`:402-405`), row 5 "Instrument review latency" (`:116`),
E1 single-agent success (`:622-628`), E2 review latency (`:630-635`), E3 discarded hours not
conflicts (`:637-640`); worker interviews (stale `next` ≈ 40%: `:86-90`; imported red:
`:96-100`, `:176-180`); coordinator interview (imported red `:24-29`).

**Verdict (one line).** Six metrics, all derivable from plan 0001's six tables **plus nine
columns and two event kinds** (listed in §3); L splits into reviewer-wait and rewind cost;
"success" gets two operational tiers (first-hand-over green; landed at first attempt without
rework) that are explicitly *not* pass@1; relay count is measurable only through the
`PostToolUse(SendMessage)` hook or an opt-in `air note relay`, otherwise reported "unmeasured".

---

## 1. Ground rules for every metric

- **Per bead, per round; the round is the comparison unit.** Every metric is emitted by
  `air metrics --round <id> --json` with its **denominator** (count of beads / landings / sessions
  it was computed over) and the list of excluded rows with reasons. A metric with denominator 0
  prints `unmeasured`, never 0 or 100 %.
- **Facts, not self-report.** Timestamps come from hooks, `air` invocations, `bd --json` fields
  (`updated_at`, `closed_at`, `started_at` exist in bd 1.2.2 — [beads-and-gastown §1](../../beads-and-gastown.md) line 40)
  and git. No model text is a fact.
- **Wall-clock is per bead, not per operator.** METR's Feb-2026 note is the caveat: with agentic
  tools developers "would often work an unrelated task while waiting for the agent to complete its
  work" so self-timed task duration is unreliable ([METR 2026](#sources)); the ledger therefore
  logs claim→hand-over→landed clock per bead and never asks anyone how long something took.
- **Store only what git/bd cannot re-derive** (plan 0001 §2 principle). Merge parents, ancestry,
  branch existence, diff vs main are derived at query time; triggers, decisions, and observation
  timestamps are stored because nothing else records them.

## 2. Metrics

### 2.1 Review latency L (per bead) — three intervals

Corpus definition: "`awaiting_review` → merged is timed" (research-actions `:116`, E2 `:630-635`);
Little: throughput = WIP/L on the review stage (fleet-size row 23).

| Symbol | From → to | Source facts |
|---|---|---|
| **L** (headline) | `t_handover` → `t_landed` | `claims.first_handover_at` → `landings.finished_at` of the landing whose `beads` contains the bead and `result = landed` |
| **L_wait** (reviewer wait) | `t_handover` → `t_land_first` | `claims.first_handover_at` → min `landings.started_at` over landings for that worker/branch with `attempt_no = 1` after `t_handover` |
| **L_rewind** (rewind cost) | `t_land_first` → `t_landed` | same landings rows; 0 when `attempt_no = 1` landed |
| **L_green** (evidence age) | `t_green` → `t_handover` | first `verify_runs` row with `exit = 0`, `kind = verify`, `sha = handover sha` → `first_handover_at` (how long a green sat before the worker asked; usually seconds — if large, the worker kept committing) |

`t_handover` = the moment the bead first became `awaiting_review`. Written by `air handover`
when it passes; when a worker bypasses Air (`bd update -s awaiting_review` by hand), reconciliation
sets `first_handover_at` from bd `updated_at` and marks `handover_source = 'bd-reconcile'` — a
different fact, printed as such. `t_landed` = `landings.finished_at` (the receipt); the merge
commit's committer date is the cross-check.

Edge cases. (a) Hand-over, more commits, hand-over again: L uses the **first** hand-over
(what the reviewer saw first); `L_last` from `claims.last_handover_at` is emitted alongside so
"the worker kept moving the target" is visible as `L − L_last`. (b) Batch land closes n beads:
each gets the same `t_landed`; the landing row lists all beads. (c) Bead handed over then
reassigned/abandoned: L is undefined → excluded, counted under §2.3. (d) Round rolls over
before landing: L is right-censored — report "n open at round end, oldest age X" rather than
dropping. (e) Owner lands from a peer's merge of the branch (bead lands inside another worker's
landing): `t_landed` = first landing whose merge commit is an ancestor-descendant of the
hand-over sha (`git merge-base --is-ancestor <handover-sha> <merge-commit>`), regardless of who
ran `air land`.

Not to be confused with: **2606.22721's latency**, defined as "hours between PR opening and
review submission (median per episode)" — that is *time to a review verdict*, per review; Air's
L is queue + service to *merged on main*, per bead. The paper's instrument transfers as three
per-reviewer-index series (the owner is the single reviewer): **approval at first attempt** =
`attempt_no = 1 ∧ result = landed`; **effort** = rewinds per landing and `L_rewind`; **latency by
reviewer experience** = `L_wait` plotted against the owner's cumulative landing index. Also not
to be confused with `air record`'s verify duration (`finish − start`), which is *build* time.

### 2.2 Single-agent success rate per bead

What the corpus means: Kim et al.'s β<0 threshold is single-agent *accuracy* (>45 %), which
"we have never measured … not once" (`:337-343`, `:578-583`); E1 (`:622-628`) defines the offline
experiment: check out the commit before the bead landed, hand one agent the bead + acceptance
criterion, "score pass/fail against that criterion alone". Benchmark analogues: **pass@k** —
"k code samples are generated per problem, a problem is considered solved if any sample passes
the unit tests, and the total fraction of problems solved is reported" (Chen et al. 2021 §2.1;
pass@1 is k = 1); **SWE-bench "resolved"** — "If the patch applies successfully and all of these
tests pass we consider the proposed solution to have successfully resolved the issue"
(FAIL_TO_PASS + PASS_TO_PASS, one attempt) ([sources](#sources)).

Air cannot run E1 in normal operation (it needs a *fresh* agent on a *replayed* checkout), so
it emits two operational tiers and names them plainly:

| Tier | Success = | Source facts |
|---|---|---|
| **S1 first-hand-over green** | the bead's **first** `air handover` invocation passed all gates (green `verify_runs` at HEAD, contains main, fitness/docs green, claim held) | `claims.first_handover_at`, `claims.handover_attempts = 1` at that moment (events kind `handover`, `decision = pass` on the first record for that bead) |
| **S2 landed clean** | landed on main at the reviewer's first attempt (`attempt_no = 1`, no rewind), and no reopen/`--reopen`/follow-up bead citing it within the round | `landings` (`attempt_no`, `result`, `beads`), bd `status` history via reconcile (`claims.release_reason = landed`), bd `close_reason` |

Denominator for both: beads with **exactly one worker over the claim's life** (`claims` rows
for the bead all carry the same `worker`; reassigned/hand-off beads excluded and counted).
Beads with a *declared executable acceptance* (plan 0002 triage) are reported as a separate
stratum — that stratum is the only one comparable to SWE-bench "resolved". Emit
`S1`, `S2`, `n_single_agent`, `n_excluded_multi_worker`, `n_with_executable_acceptance`.

Edge cases. Advisory-mode Stop hook (M0) fires `handover` records the worker did not request —
count only records with `trigger = cli|stop-blocking`, never `stop-advisory`. A bead closed
directly by `bd close` without a hand-over: S1 undefined, S2 by landing evidence only. Rewind
that was **not** the bead's fault (peer's tree, "green alone / red together" — as-built §3) still
counts against S2 (S2 is an outcome, not a blame metric); the cause is visible in `landings.failing_step` and §2.4.

Not to be confused with: **pass@1**, which is a hidden-test, independent-scorer, single-sample
number; S1/S2 are conditional on the agent's own verify command and the human reviewer, and on
the bead being well-posed (eight false premises last round, `:339-341`). Not to be confused
with **bead duration** (`:337`) — the exact substitution the corpus flags. E1 remains a separate
offline run; Air's contribution is that `landings`/`claims` give it the sample frame (closed
beads, landed sha, pre-land parent) for free. Re-run E1 and re-read S1/S2 after every model
upgrade (fleet-size gap 2).

### 2.3 Discarded work hours

Corpus: "measure hours of discarded work, not conflicts" (E3 `:637-640`; SYNTHESIS §1b).

Two components, summed and also reported separately:

- **Abandoned:** for each claim with `release_reason ∈ {abandoned, superseded, false-premise,
  reassigned}` **or** whose branch/worktree is gone with commits not ancestors of main
  (derived: `git branch --contains`, worktree list) — discarded = **worker active time inside the
  claim window**, where active time = union of `sessions` intervals in state `working|running`
  (from the `session_state` events; the `sessions` table holds only the current state) clipped
  to `[claimed_at, released_at]`. Fallback when the session series is missing: wall clock of the
  claim window, flagged `basis = wallclock`.
- **Rewound:** for each landing with `result = rewound`, discarded = worker active time between
  that landing's `finished_at` and the next landing `started_at` for the same worker/branch
  (the fix-and-retry loop), plus the land run's own duration.

Emit hours, `basis` per row (`sessions|wallclock`), and the count of claims per reason.
Edge cases: a claim held across a session restart (SessionEnd not guaranteed — hook edge-cases
tick) — cap active time at the last hook-observed activity, never at wall clock; WIP-check-point
commits that were later squashed are **not** discarded (their content landed); a bead released
because it was already fixed on main (`false-premise`) is discarded work only for the active
time spent, not for the bead's estimate. Not to be confused with **conflicts** (count of textual
merge conflicts — the "wrong outcome variable"), with **idle time** (worker waiting on review, which
is L_wait), or with agent tokens/cost (billing note; not a ledger fact).

### 2.4 Imported-red incidents

Field definition from last round: a worker merged a peer's *tip* that was red for the peer's own
reasons (7 stale matrix citations), the worker's branch went red, cost ~15–20 min
(worker `:96-100`, coord `:24-29`).

Operational: a `verify_runs` row R with `exit ≠ 0` such that (i) the worker's previous run on
the first-parent lineage of `R.sha` was green; (ii) between that green sha and `R.sha` the branch
contains a merge whose second parent P is **not** the worker's own commit and **not** on main
(derived from git); (iii) P has **no** green `verify_runs`/`fitness` row by its owner at P, or
its owner has a red row at P (or a red on P's first-parent lineage after their last green); and
(iv) the failing step is fixed by a later merge of a peer commit P′ (descendant of P) with no
edit_journal activity by the worker on the failing files in between — (iv) is a confirmation, the
incident is *provisional* on (i)–(iii) alone. Recorded as one incident per (worker, P).

Distinguish from **"green alone, red together"** (as-built §3: both parents individually green,
merge red — a semantic conflict, the case `air land` already catches twice a round) — count that
separately as `merge-red`. Distinguish from *my own* red imported by peers (fourth `:176-180`) —
same incident, attributed to the red-tip owner: report both "imported by" and "exported by".
Not to be confused with a red at hand-over (gate refusal; that is §2.2 S1) or with a rewind at
land (§2.3). Needs `verify_runs.trigger = post-merge` to know a run was the immediate post-merge
check (plan 0001 §3 `air post-merge`), and `failing_step` to match causes.

### 2.5 Stale-suggestion rate for `air next`

Field: coordinator's ready list "~40 % stale by the time I got to items 3–5" (frontend `:86-90`):
already closed, already claimed, owner-gated on read.

Every `air next` invocation logs an event `next` with the ranked list `[bead, rank, reason,
peers-flagged]` at time t. A suggestion (bead b shown to worker W at t) is **stale** when the first
subsequent fact about b is any of: (a) claimed by a worker ≠ W at t′ < W's claim (bd refused, or
`claims` shows the other actor first); (b) closed/`awaiting_review` by another worker with no claim
by W; (c) claimed by W and released with `release_reason ∈ {false-premise, superseded, owner-gated}`
without a hand-over. **Live** = W claimed b and reached hand-over, or b remained open and unclaimed
in bd at the next `next` call. Rate = stale / (stale + live) over suggestions with an outcome;
suggestions with no subsequent fact by round end are `undetermined` (reported). Compute for top-1,
top-3, top-5 separately (the interview says the top was live and 3–5 stale). Not to be confused
with the *coordinator's* chat suggestions (not observed unless §2.6 logs them) or with `bd ready`
staleness (bd is re-read live on every call; the residual is *what Air's live filter cannot see*:
false premises, owner gates, semantic overlap).

### 2.6 Coordinator relay count (merge/overlap messages)

Field: ~5 of ~30 coordinator messages per round were merge/overlap relays (plan 0001 §8; coord
interview). Air never sends messages, so the fact exists only if messages are observed. Two ways:

- **Passive (recommended):** the coordinator's own Claude Code session runs Air's hooks;
  `PostToolUse(SendMessage)` (and the worker-side receive, when a hook exists) logs an event
  `message` with `from, to, chars, ts` and a **keyword class** (`merge` if the text contains a
  sha or "merge"; `overlap` if it names a file held by a peer per `edit_journal`; else `other`).
  Message bodies are **not** stored — only class and length. Class is a heuristic; report as
  `relay_est`.
- **Opt-in:** `air note relay --about merge|overlap|bead|other [--to W]` for the coordinator to
  tag a relay when it sends one; and `air note` free-form for anything else. Rows in events kind
  `note`. Counted as `relay_tagged`.

If neither exists for a round, the metric prints `unmeasured` — never a guess. Not to be
confused with total SendMessage volume (steering, priorities, rulings are *supposed* to remain —
plan 0001 §6) or with `air merge-advice` calls (which are the replacement, and are counted
separately as `advice_calls`; the success criterion is `relay_est ↓` while `advice_calls ↑`).

### 2.7 Estimate accuracy (per bead; added 2026-08-20)

Definition: `actual / estimate` where `estimate` is the minutes recorded at filing
(`bd create --estimate`, read via `bd show --json`) and `actual` is active time from first
claim to first green hand-over (same basis as §2.3's active-time rule). Report the median ratio
and the spread per round, and the share of beads with no estimate (denominator). Never gated;
the coordinator reads it to calibrate sizing (decomposition skill). Confusions: a bead re-cut
mid-flight keeps its original estimate and is flagged `recut`; rewind loops count toward actual.

### 2.8 Capture inbox depth and time-to-triage (added 2026-08-20)

Definition: open captures at each tick (depth) and, per capture, `promoted_at - captured_at` or
`dropped_at - captured_at`. Median and p90 per round. This is the number that decides whether
triage leaves the coordinator for a dedicated session (decisions 2026-08-20).

## 3. Column and event additions to plan 0001 §2

Only what cannot be re-derived from git/bd. Everything else (merge parents, ancestry, branch
existence, holdings) stays derived.

| Table | Column | Type | Written by | Used by |
|---|---|---|---|---|
| `verify_runs` | `trigger` | enum `cli\|handover\|land\|post-merge\|stop-advisory\|stop-blocking\|selftest` | the invoking Air command | 2.2 (which hand-overs count), 2.4 (post-merge runs), 2.1 `L_green` |
| `verify_runs` | `failing_step` | text, null when green | `air record` parses the runner's END trailer / first failing target | 2.4 cause matching, 2.3 rewind cause |
| `claims` | `first_handover_at`, `last_handover_at` | ts | `air handover` on pass; reconcile from bd `updated_at` when set by hand | 2.1, 2.2 |
| `claims` | `handover_source` | enum `air\|bd-reconcile` | same | 2.1 provenance |
| `claims` | `handover_attempts` | int | `air handover` (each non-advisory invocation) | 2.2 S1 |
| `claims` | `released_at`, `release_reason` | ts; enum `landed\|abandoned\|reassigned\|superseded\|false-premise\|owner-gated\|unknown` | `air release --reason`, `air land` (landed), reconcile (`unknown` when bd changed underneath) | 2.3, 2.5 |
| `claims` | `suggested_by_next_id` | event id, nullable | `air claim` when the bead was in the last `next` output to this worker | 2.5 |
| `landings` | `attempt_no` | int, per (worker, branch) since last landed | `air land` | 2.1 `L_wait`/`L_rewind`, 2.2 S2, 2.3 |
| `landings` | `started_at`, `finished_at`, `beads` (json array), `merge_commit` | ts, ts, list, sha | `air land` | 2.1, 2.2, 2.3 |
| events | kind `next` | `{worker, ts, list:[{bead, rank, reason, flagged_peers}]}` | `air next` | 2.5 |
| events | kind `session_state` | `{worker, session, state, ts}` (every transition; `sessions` keeps only the current row) | hooks | 2.3 active time |
| events | kind `message` / `note` | see §2.6 | `PostToolUse(SendMessage)` hook / `air note` | 2.6 |

`sessions` and `edit_journal` need no new columns. `landings.result` already carries
`landed|rewound|refused` + failing step; keep `failing_step` there too. The `metrics` output is a
CLI derivation (`air metrics`), not a table.

## 4. What still cannot fall out of normal operation

- **E1 (true single-agent success on a replayed checkout)** — needs a fresh agent per bead;
  Air supplies the frame (`landings.beads`, `merge_commit^`), a script runs it. Re-run per model
  step (fleet-size gap 2).
- **Reviewer habituation** in the paper's sense needs many reviewers; with one lander Air can
  only show the owner's first-attempt approval and `L_wait` over the landing index — a trend,
  not a within-reviewer test.
- **Relay class** is heuristic without message bodies; the opt-in note is the honest count.

## Sources (accessed 2026-08-18)

- Yu, Liu, Jiang, Jia, Wang, Qian, Chen, "Habituation at the Gate: Rising Approval and
  Declining Scrutiny in Human Review of AI Agent Code", arXiv 2606.22721 (submitted 21 Jun 2026)
  — https://arxiv.org/abs/2606.22721 and https://arxiv.org/html/2606.22721 . Abstract: 400 repeat
  reviewers, 11,429 reviews, seven months; approval 30.1 % → 36.8 %; +14.5 pp first→tenth
  experience decile; "review latency increases rather than decreases (+3.5x), while inline comment
  volume decreases (−22%, p=0.0014)". Definitions: §3.1 "Review latency: hours between PR opening
  and review submission (median per episode)"; §2.4 "Approval rate (AR): fraction of reviews with
  outcome approved"; §3.2 inline comments per review 1.01 → 0.79, words 18.6 → 13.5; §2.2 early/late
  = temporal midpoint split per reviewer; deciles by within-reviewer review index; §4.1 median PR
  size flat (ρ = +0.02).
- Chen et al., "Evaluating Large Language Models Trained on Code", arXiv 2107.03374 (Jul 2021),
  §2.1 Functional Correctness — https://arxiv.org/abs/2107.03374 (PDF text extracted): "the pass@k
  metric, where k code samples are generated per problem, a problem is considered solved if any
  sample passes the unit tests, and the total fraction of problems solved is reported"; unbiased
  estimator with n ≥ k samples, c correct: pass@k := E[1 − C(n−c,k)/C(n,k)]; the naive
  1 − (1 − p̂)^k is biased (Appendix A).
- Jimenez et al., "SWE-bench: Can Language Models Resolve Real-World GitHub Issues?", arXiv
  2310.06770 (Oct 2023, rev. Nov 2024) — https://arxiv.org/html/2310.06770 : §2.2 "If the patch
  applies successfully and all of these tests pass we consider the proposed solution to have
  successfully resolved the issue"; App. A.4 resolved when "all FAIL_TO_PASS and PASS_TO_PASS tests
  are found and have a pass status"; one attempt per instance.
- METR, "We are Changing our Developer Productivity Experiment Design", 24 Feb 2026 —
  https://metr.org/blog/2026-02-24-uplift-update/ : 57 developers, 143 repos, 800+ tasks; −18 %
  (CI −38 % to +9 %) returning, −4 % (−15 % to +9 %) new; "Some developers reported it was
  challenging to report time-spent in completing tasks when they used agentic tools, because they
  would often work an unrelated task while waiting for the agent to complete its work"; signal
  called unreliable chiefly because of non-participation of developers unwilling to work without AI.
- Little 1961 via [fleet-size row 23](../fleet-size-partition-cadence.md); Kim et al. β<0 via
  [fleet-size §Gaps 2](../fleet-size-partition-cadence.md).
- Internal: plan 0001 §2/§8; SYNTHESIS §1b; coordinator interview §6(i) (`:59-61`); worker
  interviews `:86-90`, `:96-100`, `:176-180`; research-actions `:116`, `:337-343`, `:402-405`,
  `:578-583`, `:622-640`; adopter-as-built §3 (`:185`, "green alone, red together").
