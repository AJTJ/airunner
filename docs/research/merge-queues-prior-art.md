# Merge queues: prior art for a verification lane

Written 2026-09-14 for the decision to make a verification lane (one agent whose job is merging
worker branches into main after verifying them) a first-class Air role. Every claim cites a
URL with an access date, or a `path:line` in this repo at commit `aba00d8`. Where a fetch
failed the failure is stated and no content is invented.

Fetches that failed on 2026-09-14, so nothing below rests on them: Graydon Hoare's post
(https://graydon2.dreamwidth.org/1597.html, HTTP 403; web.archive.org is not reachable from
this tool); the ACM copy of the Uber paper (https://dl.acm.org/doi/10.1145/3302424.3303970,
HTTP 403) and Uber's blog copy (HTTP 404); the Gas Town design page
(https://github.com/steveyegge/gastown/blob/main/docs/design/refinery.md, HTTP 404;
https://gastown.dev/ does not resolve); Trunk's documentation sub-pages
(https://docs.trunk.io/merge-queue/parallel-queues and `.../managing-merge-queue/advanced-settings`
return a login screen); `https://raw.githubusercontent.com/rust-lang/bors/main/docs/commands.md`
(HTTP 404; the live help page was used instead).

## 1. The "not rocket science" rule, bors, homu, bors-ng, rust-lang/bors

**The rule.** "Automatically maintain a repository of code that always passes all the tests."
Graydon Hoare implemented it as the first bors in 2013 from a rule he attributes to Ben
Elliston (https://huonw.github.io/blog/2015/03/rust-infrastructure-can-be-your-infrastructure/,
accessed 2026-09-14). bors.tech restates it as "the exact integration of pull requests that end
up in the main branch have already been tested before any developers try to work on it or users
try to deploy it" and attributes it to Hoare (https://bors.tech/, accessed 2026-09-14).

**What the original did, per homu.** Test just before the merge, not when the PR arrives: "after
several other pull requests are merged into the default branch, your pull request can *still*
break things after being merged", so "the test procedure should be executed *just before the
merge*"; homu "listens to the pull request comments, waiting for an approval comment", tests the
PR, "and only when it passes all the tests, it is merged into the default branch"
(https://raw.githubusercontent.com/rust-lang/homu/master/README.md, accessed 2026-09-14). The
mechanics: on `r+`, homu "merges the PR with master into a new branch and runs tests. If tests
pass, Homu fast-forwards to the merge commit and processes the next patch in its queue"
(huonw, above). Homu is "stateful" and push-driven (webhooks), unlike the original bors which
"intends to be stateless" and polled GitHub (homu README, above).

**Ordering and batching.** Serial, one approved PR at a time, ordered by priority. Because
testing is serialized, the Rust project batches by hand: a "rollup" is one PR that merges
several approved PRs, made because "every pull request must be tested after merge before it can
be pushed to the default branch. As PR volume increases this can scale poorly, especially given
the long (~3.5hr) current CI duration for Rust"
(https://forge.rust-lang.org/release/rollups.html, accessed 2026-09-14). Rollup eligibility is
declared per PR: `rollup=always` (safe, typically docs), `maybe` (default, evaluate), `iffy`
(touches CI or bootstrap, include sparingly), `never` (same page).

**On a red.** The rollup is the unit that fails. The procedure is manual bisection: "identify the
problematic PR and unapprove it with `@bors r-`", close the rollup, recreate it without that PR
(rollups page, above). For a single PR, `retry` means "clear a failed auto build status from an
approved PR" so it is queued again (https://bors-prod.rust-lang.net/help, accessed 2026-09-14).
The rule never lets main move to a failed tree: the merge commit is built on a scratch branch and
main only fast-forwards onto a green one (huonw, above).

**Conflicts.** Not handled by the bot beyond the PR having to merge cleanly with master when
tested; the author rebases. The sources fetched do not describe an automatic conflict path.

**Flakiness.** `retry` is the whole mechanism (bors help, above).

**Priority.** `p=<n>` sets priority; `treeclosed=<priority>` closes the tree for lower-priority
PRs (bors help, above). Rollups get `r+ p=5` so they jump ordinary PRs (rollups page, above).

**Rollback.** None needed: main never holds a commit that was not tested (huonw, above).

**Unit of work.** The PR; the rollup PR when batching.

**bors-ng** (the GitHub-app descendant): "creates a merge commit, merging the main branch with
all the pull requests that make up the batch" and pushes it to the `staging` branch; when a batch
fails it uses "a strategy called 'bisecting'": "splitting the batch into two batches, and pushes
those to the queue" (https://raw.githubusercontent.com/bors-ng/bors-ng/master/README.md,
accessed 2026-09-14). "Pull requests with different priority are never batched together. The
pull request with the bigger priority number goes first"; `max_batch_size` caps a batch; `bors
single on` opts a PR out of batching. The hosted instance is being phased out and the docs point
at GitHub's native merge queue (https://bors.tech/documentation/, accessed 2026-09-14).

**rust-lang/bors** is "a rewrite of the homu bors implementation in Rust" and runs at
https://bors-prod.rust-lang.net. It uses paired branches per build kind because "we cannot set
branches to parent and merge them with a PR commit atomically using the GitHub API": try builds
on `automation/bors/try` and `automation/bors/try-merge`, auto builds on `automation/bors/auto`
and `automation/bors/auto-merge`, unrolled builds on `automation/bors/try-perf`
(https://github.com/rust-lang/bors and
https://raw.githubusercontent.com/rust-lang/bors/main/README.md, accessed 2026-09-14). Commands:
`r+`, `r=<user>`, `r-`, `p=`, `rollup=<never|iffy|maybe|always>`, `retry`, `try`, `try cancel`,
`cancel`, `delegate`, `squash`, `treeclosed=<priority>`, `info`, `ping`
(https://bors-prod.rust-lang.net/help, accessed 2026-09-14).

## 2. GitHub's native merge queue

Source: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
(accessed 2026-09-14) unless noted.

**What it does.** "The merge queue creates temporary branches with a special prefix to validate
pull request changes." A merge group is a temporary branch combining a PR with the base branch
and the PRs queued ahead of it. CI must listen for a distinct event: "You **must** use the
`merge_group` event to trigger your GitHub Actions workflow when a pull request is added to a
merge queue." The `checks_requested` activity "runs your workflow when a pull request is added to
a merge queue, which adds the pull request to a merge group"; the workflow sees `GITHUB_SHA` as
"SHA of the merge group" and `GITHUB_REF` as "Ref of the merge group"
(https://docs.github.com/en/actions/writing-workflows/choosing-when-your-workflow-runs/events-that-trigger-workflows,
accessed 2026-09-14).

**Ordering and batching.** FIFO with three knobs. Minimum group size, "useful if merges to your
base branch trigger a lengthy CI build or deploy process"; maximum group size, "useful if merges
to your base branch trigger a deployment, and you want to make sure you're not deploying too
many changes at once"; and a wait time, "a timeout for reaching the minimum group size, which
allows smaller groups to merge if there are no more PRs queued within your specified time
limit". A status check timeout says "how long the queue should wait for a response from CI
before assuming that checks have failed".

**On a red.** The failing PR is dropped, not the group rebuilt with it: "the merge queue
automatically removes pull request #1 from the merge queue" and recreates the groups behind it
without that PR. With "Only merge non-failing pull requests" enabled, "All pull requests must
satisfy required checks to be merged"; disabled, "Pull requests that have failed required checks
can be added to a group as long as the last pull request in the group has passed required
checks" (the head-of-group verdict stands for the group).

**Priority.** "Jump the queue" exists and is expensive: "jumping to the top of a merge queue will
cause a full rebuild of all in-progress pull requests."

**Conflicts, flakiness, rollback.** The page fetched does not describe a conflict path beyond
removal from the queue, has no retry mechanism of its own, and has no rollback: the base branch
only moves to a tested group head.

**Unit of work.** The PR; groups are transient.

## 3. Zuul (OpenDev/OpenStack)

Sources: https://zuul-ci.org/docs/zuul/latest/gating.html and
https://zuul-ci.org/docs/zuul/latest/config/pipeline.html (accessed 2026-09-14);
https://opendev.org/zuul/zuul/raw/branch/master/zuul/manager/__init__.py (accessed 2026-09-14).

**What it does.** A "gate" pipeline runs under the dependent pipeline manager, "designed for
gating. It ensures that every change is tested exactly as it is going to be merged into the
repository" (pipeline.html). It "allows for parallel execution of test jobs for gating while
ensuring changes are tested correctly, exactly as if they had been tested one at a time"
(gating.html).

**Ordering and batching.** Speculative: "it assumes that all jobs will succeed and tests them in
parallel accordingly." Jobs for A test A merged; jobs for B test A and B merged; jobs for C test
A, B and C merged (gating.html). Ordering is arrival order, with `Depends-On:` footers pulling a
dependency ahead of its dependent: "if change A depends on B ... B will appear first and A will
follow." Every job for one queue item sees a frozen repo state: "Zuul will freeze the repo state
(i.e., branch heads and tags) and use that same state for every job run for that queue item."

**The window.** "The window is the number of changes at the head of the queue where Zuul will
start jobs." It is TCP-style flow control: "Each time a change successfully merges, the window is
increased by one. Each time a change fails, the window is halved." Defaults: `window` 20,
`window-floor` 3, `window-increase-type` linear by 1, `window-decrease-type` exponential by 2
(pipeline.html).

**On a red, the nearest-non-failing-item rule.** "If one fails, then changes that were expecting
it to succeed are re-tested without the failed change" (gating.html). In manager terms: "If a
change near the front of the pipeline fails its tests, each change behind it ignores whatever
tests have been completed and are tested again without the change in front" (pipeline.html).
In the code, `_processOneItem(self, item, nnfi)` carries the nearest non-failing item; when
`item_ahead != nnfi and not item_ahead_merged` the comment reads "Our current base is different
than what we expected, and it's not because our current base merged. Something ahead must have
failed", jobs are cancelled with `self.cancelJobs(item)`, the item is moved behind the NNFI
with `change_queue.moveItem(item, nnfi)`, and the log says "Resetting builds for changes %s
because the item ahead, %s, is not the nearest non-failing item" (manager/__init__.py). After a
failure of C in A..E, "This queue is the same as if two new changes had just arrived, so Zuul
starts the process again" (gating.html).

**Conflicts.** A change that fails to merge is dequeued and items behind it have builds
cancelled (`self.cancelJobs(item_behind)`, manager/__init__.py). Zuul does not resolve.

**Flakiness.** Nothing automatic in the pages fetched; a `recheck` comment re-enqueues by
convention (not in the fetched pages; stated as not sourced).

**Priority.** Per pipeline, not per change: `precedence` "indicates how the build scheduler
should prioritize jobs for different pipelines" (pipeline.html).

**Rollback.** None: the branch only moves to a tested state. `dequeue-on-new-patchset` (default
true) drops a change when its author pushes a new patchset (pipeline.html).

**Unit of work.** The change (a Gerrit change or a PR), as a queue item.

## 4. Chromium Commit Queue, LUCI CV, Gerrit

**Chromium CQ.** "It's a service (aka a bot) that commits Gerrit changes for you, instead of you
directly committing the change." It is explicitly not ordered: "The commit queue is not really a
queue at the moment, since it processes the changes out of order", so "a CL can be committed
before another CL that was triggered much earlier. This can happen when a try job is flaky."
"The CQ rejects patchsets with open dependencies." "The legacy CQ Daemon is being replaced by
LUCI Change Verifier (CV)"
(https://chromium.googlesource.com/infra/infra/+/refs/heads/main/doc/users/services/commit_queue/index.md,
accessed 2026-09-14). Each CL is verified alone against tip of tree; there is no batch.

**On a red and flakiness.** The CQ runs suites "(with patch)", then "(retry shards with
patch)", then "(without patch)", and fails only tests "that fail both with-patch and
retry-with-patch but pass without-patch". "The CQ mitigates flakiness by retrying failed
tests"; a failure that also fails without the patch is treated as broken on tip of tree, not the
CL's fault (https://chromium.googlesource.com/chromium/src/+/main/docs/infra/cq.md, accessed
2026-09-14). Dry run reports without submitting; full run submits if no new regressions.

**LUCI CV** "is the LUCI microservice that is responsible for running pre-submit tests and
submitting CLs when they pass all checks"; its `internal/tryjob` component "manages tryjobs
(i.e. Buildbucket builds) which are used to verify a CL", and the test config has "combinable"
config groups that enable "multi-CL Runs in ChromeOS style"
(https://chromium.googlesource.com/infra/luci/luci-go/+/main/cv/README.md, accessed 2026-09-14).
So multi-CL verification exists as an opt-in run mode, not as the default.

**Gerrit submit requirements.** "Submit requirements are rules that define when a change can be
submitted." Each has `submittableIf` ("A query expression that determines when the change can
be submitted", mandatory), `applicableIf`, and `overrideIf` ("When this expression is evaluated
to true, the submit requirement state becomes `OVERRIDDEN`"); they replace label functions like
`MaxWithBlock` (https://gerrit-review.googlesource.com/Documentation/config-submit-requirements.html,
accessed 2026-09-14). This is a declarative gate, the same shape as Air's close gate.

**Submit whole topic.** "Gerrit may be configured to submit all changes in a topic together with
a single click, even when topics span multiple projects"
(https://gerrit-review.googlesource.com/Documentation/intro-user.html, accessed 2026-09-14);
`change.submitWholeTopic` "Determines if the submit button submits the whole topic instead of
just the current change", default false
(https://gerrit-review.googlesource.com/Documentation/config-gerrit.html, accessed 2026-09-14).

**Submit types** decide what landing does to commit identity: Fast Forward Only ("a change can
only be submitted if at submit time the target branch can be fast-forwarded to the commit");
Merge If Necessary ("fast-forwards the target branch if possible, and otherwise creates a merge
commit automatically"); Merge Always; Rebase If Necessary ("automatically rebases the current
patch set of the change on top of the current head", new commit); Rebase Always; Cherry Pick
("a brand new commit on top of the current head", submitter becomes committer, author kept,
"ignores the parent lineage")
(https://gerrit-review.googlesource.com/Documentation/config-project-config.html, accessed
2026-09-14).

**Unit of work.** The CL / change; the topic when submit-whole-topic is on.

## 5. Uber's SubmitQueue (Ananthanarayanan et al., EuroSys 2019)

Primary copies were not fetchable (see the failures list). Sources used: the author's slides,
https://sundaram.io/slides/eurosys19.pdf (accessed 2026-09-14; text extracted with
`pdftotext`, the evaluation slides are images and yielded no text), and the Morning Paper
summary, https://blog.acolyer.org/2019/04/18/keeping-master-green-at-scale/ (accessed
2026-09-14), which quotes the paper.

**Problem.** A single serial queue "guarantees an always green master by serializing changes"
but "does not scale to 1000s of changes/day"; plain batching "improves the throughput if batches
succeed more often than not" (slides). The goal is "illusion of a single queue when committing
changes" with "overheads ... short enough for developers to trade speed for correctness"
(slides). Individual iOS/Android builds took 30 to 60 minutes and mainline uptime before the
system was 52% (acolyer).

**Design.** Three parts: a Speculation Engine that "speculates on success/failure of changes"
and "builds speculation graph"; a Conflict Analyzer that "determines independent changes" and
"constructs conflict graph"; a Planner Engine that "selects most valuable builds from
speculation engine" and executes them (slides). The speculation tree is "a binary decision
tree ... annotated with prediction probabilities for each edge"; with independent changes "the
speculation tree can become a speculation graph. This enables independent changes to be
committed in parallel" (acolyer). Conflict is defined on build targets: "two changes conflict if
they both affect a common set of build targets", each target carrying "a unique target hash"
(acolyer). Build selection is by "predicted value – which is a combination of likelihood of
success and change priority"; the models are "logistic regression" over "100+ hand-picked
features" (number of affected targets, commits, files, presubmit status, developer name, past
speculation outcomes) with "prediction accuracy of 97%" (slides; acolyer).

**On a red.** A failed build rejects the change it was speculating for ("Build steps for H ⊕ C2
fails and C2 is rejected"; "B1 fails → C1 rejected", slides); the planner re-plans and "stops
execution of any currently running builds not included in the new schedule" (acolyer).

**Measured.** Post-implementation "100% green" mainline and a turnaround "1.2x of the Oracle"
(the oracle knows every outcome in advance) (acolyer). Throughput and wait-time figures are on
image slides and were not extractable; not quoted here.

**Unit of work.** The change (a diff against a monorepo).

## 6. Products: Mergify, Aviator, Graphite, Trunk, Google TAP

**Mergify.** Batching combines PRs into one CI run: "Batch merging lets you combine and merge
multiple pull requests at once"; `batch_size` sets how many, `batch_max_wait_time` how long to
wait for a batch to fill. On failure it does not discard the batch: it splits it "into parts
(dictated by `max_parallel_checks`, minimum two)", retests each part, "and repeats recursively
until isolating individual culprits", capped by `batch_max_failure_resolution_attempts`
(https://docs.mergify.com/merge-queue/batches/, accessed 2026-09-14). Speculation is
cumulative draft PRs: "Mergify creates temporary batch PRs that represent cumulative merges (PR
#1), (PR #1 + PR #2), (PR #1 + PR #2 + PR #3), runs CI on them in parallel"; "If a batch fails,
Mergify removes the culprit PR and continues with the rest"
(https://docs.mergify.com/merge-queue/speculative-checks/, accessed 2026-09-14). Conflicts are
caught by re-basing before merge: "Mergify updates each PR against the latest main and re-runs
CI before merging"; priority via labels or rules ("Let urgent PRs jump the queue")
(https://docs.mergify.com/merge-queue/, accessed 2026-09-14). Unit: PR.

**Aviator MergeQueue.** Serial by default: on ready-to-merge it "tests the pull request against
the latest changes in the target branch and merges the pull request only if it passes all the
required checks"; the stated problem is "semantic conflicts" between PRs that pass alone
(https://docs.aviator.co/mergequeue, accessed 2026-09-14). Parallel mode builds a draft PR per
queue position "with changes from both `PR #1` and `PR #2`" and so on, bounded by a max parallel
builds setting after which "the bot will pause queuing PRs". On a draft PR failure, if it is
first with nothing behind, the original PR is dequeued; otherwise "the bot closes all subsequent
Draft PRs and restarts the queue after removing the failing PR"; a stalled original PR gets a
stuck label and a timeout (https://docs.aviator.co/mergequeue/concepts/parallel-mode, accessed
2026-09-14). Batching: `batch_size` "Defaults to 1", `batch_max_wait_minutes`; on failure "we
will close draft PR #6, and requeue all of PRs #1-5. These PRs will be put into two bisected
batches" (https://docs.aviator.co/mergequeue/concepts/batching, accessed 2026-09-14). Unit: PR.

**Graphite.** Stack-aware: "if the stack is added to the queue together, the merge queue can
process and validate the entire stack in parallel"; strategies are Rebase ("commits unchanged")
and Squash, with an optional "fast-forward merge" to process stacked PRs in parallel
(https://graphite.com/docs/graphite-merge-queue, accessed 2026-09-14). "Parallel CI uses
speculative execution, similar to branch prediction, to run CI for multiple enqueued stacks at
the same time" on `gtmq_` branches; batching is "available in private beta". On a batch
failure there are two culprit-finding modes: "Full parallel isolation (default): By checking
every stack in the batch in parallel, the problematic stacks are identified quickly" and
"Bisection: By using a bisection approach, problematic stacks are identified efficiently with
fewer CI runs"; failed stacks are "removed from the queue", passing ones "automatically added
back" (https://graphite.com/docs/merge-queue-optimizations, accessed 2026-09-14). Unit: PR or
stack.

**Trunk Merge Queue.** Predictive testing: each PR is tested "against the head of `main` plus
every PR ahead of it — so what merges is what was actually tested"; batching; "Lanes for
unrelated PRs" by impacted targets; "Anti-flake protection" where "CI gets a second chance";
fast-track "to the front of the queue without bypassing it"
(https://docs.trunk.io/merge-queue, accessed 2026-09-14). "If a batch fails, automatic
bisection isolates the culprit and the healthy PRs keep moving"; "a PR that fails on a flake
stays in line while downstream PRs test, and merges once a later one passes"
(https://trunk.io/merge-queue, accessed 2026-09-14). Priority is "a number from 0 to 255 or
... `urgent`, `high`, `medium`, or `low`"; "Trunk Merge tries to never interrupt testing in
progress because that would be very inefficient", except "An urgent PR will interrupt a
currently testing PR and begin immediately"; "Optimistic Merging" lets a PR merge "even when it
failed, but only in the case where another PR that depends on the first does pass"
(https://trunk.io/blog/trunk-merge-perf-and-feature-updates, accessed 2026-09-14). The
configuration reference pages are behind a login and were not read. Unit: PR.

**Google TAP.** "Every day it is responsible for handling more than 50,000 unique changes _and_
running more than four billion individual test cases." Presubmit runs a fast subset and
"a change that passes the presubmit has a very high likelihood (95%+) of passing the rest of the
tests, and we optimistically allow it to be integrated"; average wait about 11 minutes.
Postsubmit batches changes, and "TAP automatically splits a failing batch up into individual
changes and reruns the tests against each change in isolation", with binary-search tooling for
developers; rolling back remains the fastest recovery
(https://abseil.io/resources/swe-book/html/ch23.html, accessed 2026-09-14). The ICSE 2017 paper
page offers only an abstract, which notes "very few of our tests ever fail" and that code
"recently modified by multiple developers (more than 3) breaks more often"
(https://research.google/pubs/taming-google-scale-continuous-testing/, accessed 2026-09-14).
TAP is a postsubmit culprit-finder over an optimistic presubmit, not a merge queue: main can be
red and is rolled back. Unit: the change.

## 7. Agent-fleet merge queues

**Gas Town's Refinery.** Already summarised in
[`beads-and-gastown.md`](beads-and-gastown.md) §2.4 (`docs/research/beads-and-gastown.md:148-150`):
batch-then-bisect, pluggable gates, an integration branch per epic, conflicts become a task for
another polecat, and `bd merge-slot` serialises conflict resolution. Confirmed from the design
doc on 2026-09-14: "Rebase A..D as a stack on main", "Run tests on D (tip of stack)", pass gives
"Fast-forward merge all 4 → done", fail gives "Binary bisect → test B (midpoint)"; "Gates (test
command, lint, etc.) are pluggable. The batching strategy is core."
(https://raw.githubusercontent.com/steveyegge/gastown/main/docs/design/architecture.md,
accessed 2026-09-14). The worker side: `gt done` "Pushes branch, submits MR to merge queue" and
the Refinery does "Rebase and merge to target branch (main or integration branch)", "Close the
issue", and "If conflict: create task for available polecat"
(https://raw.githubusercontent.com/steveyegge/gastown/main/docs/concepts/polecat-lifecycle.md,
accessed 2026-09-14). The mutex: "Merge-slot gates serialize conflict resolution in the merge
queue. A merge slot is an exclusive access primitive: only one agent can hold it at a time", to
prevent "monkey knife fights where multiple polecats race to resolve conflicts and create
cascading conflicts"; one slot bead per rig, `create | check | acquire [--wait] | release`, with
a priority-ordered waiters queue (https://beads.gascity.com/cli-reference/merge-slot, accessed
2026-09-14). Unit: the MR bead, which names a branch. Note the rebase: the stack is rewritten
onto main before testing, so the shas the worker produced are not the shas that land.

**Overstory's conflict ladder.** "A FIFO merge queue with 4-tier conflict resolution merges
agent branches back to canonical"; "Merge: FIFO merge queue (SQLite-backed) with 4-tier conflict
resolution"; a "Merger" agent type, "Branch merge specialist", read-write; "Each agent gets an
isolated git worktree"; `ov merge` with `--branch`, `--all`, `--into`, `--dry-run`; the `merge/`
module is "FIFO queue + conflict resolution + sentinel-file lock + dry-run prediction"
(https://raw.githubusercontent.com/jayminwest/overstory/main/README.md, accessed 2026-09-14).
The tiers are Clean, Auto, AI-Resolve, Reimagine (same README, tiered description). The
author's own steelman of the critique: "Textual conflicts (both agents edit line 47) are
annoying but mechanical. **Semantic conflicts** are worse: changes that don't textually conflict
but break correctness"; "Overstory's tiered merge resolution helps, but tier 4 (AI resolver)
still requires human review for semantic conflicts"; "The merge queue becomes a bottleneck. For
highly interconnected codebases, you spend more time resolving conflicts than you saved via
parallelism" (https://raw.githubusercontent.com/jayminwest/overstory/main/STEELMAN.md, accessed
2026-09-14). Unit: the agent branch.

**Appendix A of the landscape, grepped for merge/queue/land/refinery/integration.** Projects
that ship a merge role or queue, all read from
[`harness-and-orchestrator-landscape.md`](harness-and-orchestrator-landscape.md):

| Project | What it ships | Where |
|---|---|---|
| gastown | Bors-style merge queue, 20 to 30 agents | `:326-330`, `:588-592` |
| orc | one engineer per bead per worktree, "an ephemeral reviewer per bead, and automatic review before merge to the goal branch"; no ledger, "review is an agent's opinion" | `:74`, `:570-576` |
| bernstein | "spawns agents in separate git worktrees ... verifies, merges to main. No LLM in the coordination loop: scheduling is plain Python" | `:192-199` |
| Aperant | "automatic conflict resolution on merge back to main" | `:1091-1093` |
| Fletch | the opposite policy: "Nothing merges without you. Live diffs, explicit approval gates, and your review sit between the agents and the merge" | `:1060-1066` |
| aGiTrack | "surfaces a `merge` command at the top whenever a worktree still holds un-integrated work", the same signal as Air's hand-over gate | `:967-971` |
| parallel-code | "Dispatch in parallel, review the diffs, merge the wins, toss the rest" | `:1142-1143` |

None of these is a library a Rust binary could call; each is a queue inside its own runtime.
Overstory is Bun/TypeScript (README, above), Gas Town is Go (`beads-and-gastown.md:170`).

## 8. Git mechanics a verification lane would lean on

**`git merge-tree --write-tree`.** "Perform merge without touching index or working tree"; it
"performs a merge, but does not make any new commits and does not read from or write to either
the working tree or index", using the same machinery as `git merge` (three-way content merges,
rename detection, directory/file conflicts, recursive ancestor consolidation). Exit 0 is a
clean merge and prints the tree OID; exit 1 is a conflicted merge and prints the tree OID, then
conflicted-file lines `<mode> <object> <stage> <filename>`, then informational messages
(`CONFLICT (rename/delete)`, `CONFLICT (binary)`, and so on; `-z` gives structured records).
`--merge-base=<tree-ish>` pins the base; `--stdin` batches many merges in one process. The doc's
own example then uses `git commit-tree $NEWTREE -p $BRANCH1 -p $BRANCH2` and `git update-ref` to
land without a checkout (https://git-scm.com/docs/git-merge-tree, accessed 2026-09-14). This is
the tool for "would these N branches merge cleanly with main and with each other" before a
worktree is touched: at n=4 that is 4 merges against main and 6 pairwise, all in one `--stdin`
call.

**`git worktree`.** "A git repository can support multiple working trees, allowing you to check
out more than one branch at a time"; linked worktrees share "everything except per-worktree
files such as HEAD, index, etc."; `add` "refuses to create a new worktree when <commit-ish> is
a branch name and is already checked out by another worktree"; `remove` takes "only clean
worktrees"; `lock` protects a worktree from `prune`
(https://git-scm.com/docs/git-worktree, accessed 2026-09-14). Consequence: a lane cannot check
out `worktree-<name>` while the worker has it; it merges the branch ref into its own branch, or
builds the tree with `merge-tree` and never checks the worker's branch out at all.

**Fast-forward only versus merge commits.** `--ff-only`: "resolve the merge as a fast-forward
when possible. When not possible, refuse to merge and exit with a non-zero status." `--no-ff`:
"create a merge commit in all cases." A fast-forward means "the `HEAD` (along with the index) is
updated to point at the named commit, without creating an extra merge commit"; a true merge
produces "a merge commit that has both of them as its parents." On conflict, `MERGE_HEAD` is
set, stages 1/2/3 are recorded in the index, and an `AUTO_MERGE` ref is written under the `ort`
strategy (https://git-scm.com/docs/git-merge, accessed 2026-09-14). The Air-relevant property:
`--ff-only` onto a commit built elsewhere is an atomic ref update that cannot conflict
(`crates/cli/src/cmd/audit.rs:193`), which is why `air land` uses it (`crates/cli/src/cmd/land.rs:932-939`).

**Rebase and attribution.** Rebase replays commits as new objects ("Replay the commits, one by
one, in order. This is similar to running `git cherry-pick` for each commit"; the diagram
shows A'..C'); "Rebasing (or any other form of rewriting) a branch that others have based work
on is a bad idea: anyone downstream of it is forced to manually fix their history";
`--committer-date-is-author-date` and `--reset-author-date` exist because the committer date
otherwise becomes now; the merge backend re-opens the message after a conflict, the apply
backend "blindly applies the original commit message"
(https://git-scm.com/docs/git-rebase, accessed 2026-09-14). Gerrit's Cherry Pick submit type
makes the same trade explicit: "The submitter becomes the committer; original author is
retained" (§4). For Air the cost is not only history: greens are keyed by commit sha, or by tree
when `verify_key` is `tree` (`crates/cli/src/cmd/green.rs:9-27`), and a rebase gives every
worker commit a new sha and, once rebased onto a moved main, a new tree, so a worker's own
recorded green stops matching. `Bead:` trailers survive a rebase because the message is kept
(rebase doc, above), so attribution by trailer (`crates/cli/src/cmd/batch.rs:130-141`) survives;
the evidence link does not.

**`git rerere`.** For "relatively long lived topic branches" where "the developer sometimes
needs to resolve the same conflicts over and over again"; with `rerere.enabled`, `git merge`
"automatically invokes `git rerere` upon exiting with a failed automerge and `git rerere`
records the hand resolve when it is a new conflict, or reuses the earlier hand resolve when it
is not"; `--rerere-autoupdate` also stages the result. Limits: it "relies on the conflict markers
in the file to detect the conflict" and cannot track "conflicting submodules"
(https://git-scm.com/docs/git-rerere and https://git-scm.com/docs/git-merge, accessed
2026-09-14). Useful to a worker who re-merges main after each batch; not useful to a lane that
by rule does not resolve (§9).

## 9. What transfers to a fleet of 3-4 agent branches on one machine

Scale assumptions, all from this repo: three or four worker branches per round
(`CLAUDE.md:35`, "today: one coordinator + three workers"); a verify of minutes, not seconds (the
adopter's lane ran "a batch verify of 1622 s", `docs/notes/2026-09-06-timing-budgets.md:95`;
their harness cap is 15 minutes, `docs/rules/adopting-air.md:210-213`); one machine, no CI
server; the lane is itself a Claude Code session (`docs/rules/roles.md:147`).

**What matters at this scale.**

1. *Batch, then bisect.* Every system in §1-§7 that has a slow verify batches (bors-ng,
   rollups, GitHub groups, Mergify, Aviator, Graphite, Trunk, TAP, Gas Town), and every one
   that batches splits on red rather than discarding the batch or retesting it whole. With
   n=4 and a 10-minute verify, serial landing costs up to 40 minutes; one batch costs 10; a red
   batch costs 10 plus at most two halvings (bors-ng README, §1) to name the culprit. The
   break-even is Uber's one-liner, batching "improves the throughput if batches succeed more
   often than not" (§5), and at n=4 the batch is the whole ready set, so there is no batch-size
   knob to tune.
2. *One verify at a time.* Zuul's window, Mergify's `max_parallel_checks`, Aviator's max
   parallel builds and Graphite's parallel CI all assume spare CI capacity. On one machine
   whose CPU is the verify, parallel speculation is contention, and the reported durations
   stop meaning anything (the 1622 s above was a single batch). This is Zuul with `window`
   fixed at 1 and the `serial` manager (§3), which is also exactly bors: a single staging
   branch at a time.
3. *Main only moves onto a verified tree.* bors (§1), GitHub (§2), Zuul (§3), Gerrit's
   Fast Forward Only (§4), Gas Town's fast-forward after the tip passes (§7), and `air land`
   (`crates/cli/src/cmd/land.rs:7-11`, `:915-939`) all agree; TAP is the one exception and pays
   for it with rollbacks (§6). Air already has this, so rollback stays absent by construction
   (`crates/cli/src/cmd/land.rs:238-242`).
4. *A red batch lands nothing and names its members.* Air already records members with the
   run and reports a red by member (`crates/cli/src/cmd/record.rs:42-52`, `:205-211`;
   `crates/cli/src/cmd/batch.rs:354-368`). What the prior art adds is the next step, which Air
   leaves to "the lane splits by hand" and notes "Auto-bisect waits for a count of these"
   (`crates/cli/src/cmd/batch.rs:355`; `crates/cli/src/cmd/handover.rs:534-541`).
5. *Conflicts are not the queue's job.* bors requires a mergeable PR; GitHub removes the PR;
   Zuul dequeues on merge failure; Gas Town hands the conflict to a polecat under a mutex;
   Overstory is the exception and its own steelman says tier 4 "still requires human review"
   (§7). Air's rule already matches the majority: "a branch that conflicts is dropped from the
   batch and named, never resolved by the lane" (`docs/rules/roles.md:176-177`). Detection can
   move earlier and get cheaper with `merge-tree --write-tree` (§8): a branch that conflicts
   with main or with another ready branch is known before the lane's worktree is touched.
6. *A flake gets one retry, then it is a red.* Chromium retries shards and compares against
   "without patch" (§4); bors has `retry` (§1); Trunk gives "a second chance" (§6). Air records
   a `flaky-at-head` flag on the run (`crates/cli/src/cmd/record.rs:203`); the lane policy is
   one re-run of the same batch head before splitting, and never more, since every re-run is
   the whole machine for ten minutes.
7. *Order is oldest first; priority is a bypass, not a queue discipline.* At n=4 a priority
   scheme is overhead; what survives is bors's `treeclosed` / Trunk's `urgent` idea that one
   named branch may go alone next. Air already lands `--all` "oldest first, stopping at the
   first red" (`crates/cli/src/main.rs:201`) and `--worker <name>` selects one branch
   (`:175-183`).

**What does not transfer.** Probabilistic speculation over a conflict graph (§5) needs
thousands of changes a day and spare builders; the 97% model exists to avoid building the
whole tree of outcomes, which at n=4 is three extra merges. Cumulative draft PRs (Mergify,
Aviator, Graphite, §6) exist because CI is remote and parallel. Impacted-target lanes (Trunk)
need a build graph that says which tests a change reaches; `make verify` here runs everything.
Per-PR rollup eligibility (`rollup=iffy`) encodes human judgement about CI-touching changes
that at this scale the coordinator says in a sentence. Rebase-as-a-stack (Gas Town, Graphite)
is a cost without a benefit here: it rewrites shas that Air's ledger keys on (§8) for a linear
history nobody reads.

**Unit of work.** The branch. Air already ruled it: "The branch is the unit, and `--worker
<name>` names it" (`crates/cli/src/main.rs:175`), because a bead can sit on two branches at
once (the lane's and the worker's) and only the branch is unambiguous. The prior art's PR,
change and MR bead are each a branch with a name attached; Graphite's stack is a branch chain.
Beads stay the attribution unit via `Bead:` trailers (`crates/cli/src/cmd/batch.rs:130-141`).

**Queue state, and where it lives.** The prior art keeps queue state in the bot's database
(homu is "stateful", §1; Overstory is "SQLite-backed", §7; GitHub and Zuul hold it server-side)
and the batch itself in a git ref (`staging`, `automation/bors/auto`, `gtmq_*`, a merge group
branch). Air's version is already derived rather than stored: batch-ready is a pure rule over
git and the ledger (`crates/cli/src/cmd/status.rs:511-574`), the batch is a commit on the
lane's branch that contains main plus the member heads (`docs/rules/roles.md:155-157`), the
verdict is a `verify_runs` row carrying `members` and `main_sha`
(`crates/cli/src/cmd/record.rs:42-52`), and the landing is a `landings` row
(`crates/cli/src/cmd/land.rs:1-24`). Two facts are missing and both are one row each, in the
ledger, not in a file: *dropped from batch, reason conflict, at sha* (today it is said in prose
to the worker, `roles.md:176-177`, and recorded nowhere), and *retried once at sha* (so a
second red is distinguishable from a first). Nothing needs a new table or a file; the git ref
for the batch already exists as the lane's branch head.

**Deterministic program versus the lane's judgement.** Deterministic, and therefore Air's:
which branches are ready (`status.rs:511`); whether each merges cleanly with main and with the
others (`merge-tree --write-tree --stdin`, §8); the batch order (oldest first); building the
batch commit (`commit-tree`, which `air land` already does for one branch, `land.rs:975-990`);
recording the run with members; the split on red (halve by member, oldest half first, and a
one-member batch that is red is the culprit); landing by `--ff-only`; refusing a lane that
holds a claim while batching (`roles.md:147-148`). Judgement, and therefore the agent's: reading
a red log to say whether it is a flake before spending the one retry; deciding that a red
member is worth a conflict-free rebuild versus telling the worker; and conflict resolution
itself, which the prior art unanimously keeps out of the queue and Air's rule sends back to the
worker. The one thing an LLM lane adds over bors is that it can read the log; everything else
it does bors did in 2013 without one, so the design question do-less asks is whether the lane
is a program with a session watching it, rather than a session running a program.

**The failure policy the prior art converges on.** On a red batch: land nothing; keep main
where it is; split the batch, never rebuild it with the culprit in; re-queue the innocents
without asking their authors; name the culprit to its author with the log; allow one retry for
a suspected flake; a branch that cannot merge is out of the batch before verification starts,
and its author resolves it, under a mutex if two might resolve at once (§1, §2, §3, §6, §7).
The only dissent is TAP's optimistic postsubmit with rollback, which presumes a 95% presubmit
and 50,000 changes a day (§6).

**Summary table.**

| Mechanism | Who does it | What Air would have to build | Source |
|---|---|---|---|
| Main only fast-forwards onto a verified tree | Air (`air land`) | Nothing | `land.rs:915-939`; bors §1; GitHub §2 |
| Batch-ready list | Air (`air status`) | Nothing | `status.rs:511-574` |
| Batch commit containing main + members | Lane, by hand (`git merge`) | A deterministic `air batch cut` that merges the ready set (or builds it with `merge-tree` + `commit-tree`), optional | `roles.md:155-157`; bors-ng README §1 |
| Conflict pre-check before the batch | Nobody today; lane discovers on merge | `merge-tree --write-tree --stdin` over ready branches against main and pairwise; record "dropped: conflict" as a fact | git-merge-tree §8; `roles.md:176-177` |
| Verdict recorded with members and main_sha | Air (`air record verify`) | Nothing | `record.rs:42-52` |
| Red reported by member | Air | Nothing | `record.rs:205-211`; `batch.rs:354-368` |
| Split on red | Lane, by hand | A deterministic halving that names the next batch to cut; "auto-bisect waits for a count" | `batch.rs:355`; bors-ng §1; Mergify §6 |
| One retry for a flake | Lane's judgement | Record the retry on the run so a second red is a second red | `record.rs:203`; Chromium §4; bors `retry` §1 |
| Re-queue the innocents | Lane, by hand | Falls out of the batch-ready rule once the culprit is named | Zuul NNFI §3; GitHub §2 |
| Conflict resolution | The worker, never the lane | Nothing; the mutex Gas Town needs is unnecessary at one lane | `roles.md:176-177`; Gas Town §7 |
| Priority / go-alone | Coordinator (`air land --worker`) | Nothing | `main.rs:175-183`; Trunk urgent §6 |
| Close on the lane's green | Air (close gate) | Nothing | `batch.rs:1-30` |
| Lane as a role | Prose in roles.md; `verify_lane` key read nowhere | Decide whether the key is read at all; today the flow works without it | `roles.md:145-180`; round log `:113` |

## check-resources

**Harness.** Claude Code provides the worktree half and none of the queue half: `--worktree`
creates `.claude/worktrees/<name>` on branch `worktree-<name>`, subagents can run with
`isolation: worktree`, exit prompts keep or remove a worktree with work in it, a periodic sweep
removes unchanged ones, and isolation blocks edits and git redirects into the main checkout
(https://code.claude.com/docs/en/worktrees, accessed 2026-09-14). Cross-session messaging can
"tell the other sessions what landed" (https://code.claude.com/docs/en/cross-session-messaging,
accessed 2026-09-14). The desktop app's "Auto-merge" merges a GitHub PR by squash once checks
pass, which needs GitHub and its checks (https://code.claude.com/docs/en/desktop, accessed
2026-09-14). A grep of https://code.claude.com/docs/llms.txt for merge, queue, land and
integrate on 2026-09-14 found no local merge, batch or landing feature.

**Field.** Every hosted queue (§1, §2, §6) needs a PR host and a CI server; the two agent-fleet
queues are Gas Town's Refinery in Go and Overstory's SQLite FIFO in Bun (§7), neither callable
from a Rust binary and both carrying their own runtime. The policies transfer (batch, split on
red, one retry, conflicts go back to the author, main only fast-forwards); no code does.

**Air.** Today: `air land` builds the landing commit with `commit-tree` off main and
fast-forwards with `--ff-only`, refusing unless the branch contains main and re-checking that
main has not moved (`crates/cli/src/cmd/land.rs:915-1010`); `--all` lands oldest first and stops
at the first red, `--worker <lane>` lands a lane's batch with every bead its range names
(`crates/cli/src/main.rs:159-210`); `air status` lists batch-ready branches by a pure
three-fact rule (`crates/cli/src/cmd/status.rs:511-574`, `:2543-2547`); `air record verify`
stores the batch members and main's sha with the run and reports a red batch by member
(`crates/cli/src/cmd/record.rs:42-52`, `:205-211`); the close gate accepts a lane's green at a
verified descendant (`crates/cli/src/cmd/batch.rs:1-30`); the lane role is prose
(`docs/rules/roles.md:145-180`). Absent: any conflict pre-check (`crates/cli/src/git.rs` has
`unmerged_files` at `:216` and no `merge-tree`), any automatic split on red
(`crates/cli/src/cmd/batch.rs:355`), a recorded "dropped from batch" fact, and any reader of
`verify_lane` (`docs/notes/rounds/2026-09-06-air/2026-09-06-round-log.md:113`; this repo's
`.claude/air.json` has no such key).
