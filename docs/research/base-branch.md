# A base branch other than main

Research for air-zu3, 2026-11-02, read against this tree at `e5b39bd` (the running binary is not
what was counted; every count below is `grep` over the source). Source: an adopter's
`air-updates.md` entry of 2026-10-28, "how the lane is told, and a base branch other than
main", as quoted on the bead; the owner's framing on 2026-11-02 was that this smells like
remaking git, that there may be prior art, and that the do-less question is in scope.

The short answer: git already does this, at no cost to Air, because Air reads and moves local
refs only and never names a remote. An adopter who wants the fleet to land onto a feature
branch clones with that branch as local `main` and pushes it back under its real name. The
one-key version inside Air is small but touches every gate, and no adopter has recorded a
failure it would remove, so the recommendation is to document the git answer and build nothing.

## 1. What the tools around git already do

Every merge queue keys its queue by target branch and builds a temporary "latest target plus
the members" to test, which is what `air batch cut` does locally. None of them hard-codes the
default branch, and none makes "which branch" a global setting: it is a property of each item
queued. All pages accessed 2026-11-02.

- **GitHub merge queue.** Enabled per base branch: "Repository administrators can require a
  merge queue by enabling the branch protection setting "Require merge queue" in the
  protection rules for the base branch." The queue builds "a `merge_group` with the latest
  version of the `base_branch` as well as changes from pull requests ahead of it in the
  queue", on temporary branches with the prefix `gh-readonly-queue/{base_branch}`, and merges
  "into the `base_branch` once the checks required by the branch protections of `base_branch`
  pass". A member that conflicts with the base after regrouping "will be removed from the
  queue". Wildcard branch patterns cannot carry a queue. The page never restricts the queue to
  the default branch.
  https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
- **Mergify.** A condition attribute `base`, "The name of the branch the pull request should be
  pulled into"; "A pull request goes to the first queue rule whose `queue_conditions` it
  matches", and "When no rule's `queue_conditions` match, the pull request is not queued,
  because there is no fallback queue." A queue per branch is therefore a configuration the
  user writes with `base = <branch>`, not a built-in.
  https://docs.mergify.com/configuration/conditions/ and https://docs.mergify.com/merge-queue/rules/
- **bors-ng.** "The batcher will use the github APIs to create a merge commit with all PR's
  commits and apply it to the `staging` branch. Previous `staging` changes are destroyed by
  force-pushing the merge commit over the top of it." On green it merges the merge commit. The
  flow document says nothing about the target branch or a failed batch; the forum thread that
  reportedly says batches are grouped by target branch returned HTTP 404 on 2026-11-02, so that
  claim is unverified here. The repository has been archived since 2024-04-04.
  https://github.com/bors-ng/bors-ng/blob/master/FLOW_EXAMPLE.md
- **Zuul.** "A gating system should always test each change applied to the tip of the branch
  exactly as it is going to be merged." It tests speculatively ("it assumes that all jobs will
  succeed and tests them in parallel accordingly"), and on a failure "changes that were
  expecting it to succeed are re-tested without the failed change". Queues are "shared change
  queues" of projects; the page says nothing about branches.
  https://zuul-ci.org/docs/zuul/latest/gating.html
- **Trunk-based development** is the prior art on the *practice*, not the mechanism. A release
  branch "should not receive continued development work", takes cherry-picks from trunk only
  ("You should not fix bugs on the release branch in the expectation of cherry-picking them
  back to the trunk"), and "Release branches are **not** merged back into trunk." A feature
  branch "should only last a couple of days" with "the developer count ... at one (or two if
  pair-programming)"; "other developers (or their coding agents) joining you on your
  short-lived feature branches" is listed under "Breaking the contract", and a branch several
  people merge into is "long-lived feature branches, just spread sideways rather than
  lengthways".
  https://trunkbaseddevelopment.com/branch-for-release/ and
  https://trunkbaseddevelopment.com/short-lived-feature-branches/

What applies to a lane that lands locally without a remote: the shape Air has (reset to the
base, merge the members, verify once, fast-forward the base) is the shape every queue has, and
the base is a parameter of the queue in every one of them. What does not apply: a queue per
target branch, because Air has one lane and one queue. What the practice says: three workers
landing onto one feature branch for days is the thing trunk-based development names as its
antithesis. The adopter's entry proposes exactly that.

## 2. What exists already: the local branch is the base

Air names no remote. `grep -rn '"origin"\|origin/\|"push"\|"fetch"' crates/*/src`, excluding
tests, matches nothing (2026-11-02). Every git call reads or moves a local ref, and the roles say
"Air pushes nothing; a landing reaches main and stops there". So "main" in Air is the name of a
local branch, and git lets a local branch track any upstream branch under any name:

```
git clone -b feature --single-branch <url> project   # local branch "feature"
cd project && git branch -m feature main             # now local "main", upstream still "feature"
git branch --set-upstream-to=origin/feature main
… air init / air install, run the fleet, land onto local main as usual …
git push origin main:feature                         # the owner, when the feature is done
```

Nothing in Air changes, no gate changes meaning, and the owner's push decides where the work
goes. The cost is the one every alternative shares: the feature branch is a long-lived branch
several agents merge into, and its eventual landing onto the real main is a merge no Air green
covers (see §4). A second cost is cosmetic: `main moved to <sha>` and the roles' "merge main"
mean the feature branch, which is what the adopter wanted.

## 3. The smallest shape inside Air, counted

The bead's figure, "about 160 non-test lines across 23 files", came from a plain `grep "main"`
and mixes four senses of the word. Counted on this tree with a script that drops `tests/`
directories, each file's `#[cfg(test)]` tail, comment-only lines and `fn main`:

| What | Lines | Files |
|---|---|---|
| any `main` as a whole word | 536 | 25 |
| inside a string literal | 260 | 21 |
| the bare literal `"main"` | 108 | 17 |
| of those, in `selftest.rs` (temp-repo fixtures and the `main` session name) | 80 | 1 |
| bare `"main"` elsewhere | 28 | 15 |

The 28 split by sense, and the split is the whole difficulty:

- **The branch, 15 sites in 9 files.** `git.rs:181` (`main_tip`), `handover.rs:37,41,44`
  (the close gate's three facts: main is an ancestor, main's sha, work already in main),
  `record.rs:54` (`verify_runs.main_sha`, the main a green was recorded over),
  `batch.rs:214,234,264` (a bead's commit on main; a green contains main; the batch-ready
  rule's "landable" test), `batch_cut.rs:119,284` (the cut resets to main and names a conflict
  "with main"), `land.rs:1005` (the main checkout must have main checked out, because the
  fast-forward is `merge --ff-only` there), `status.rs:1132,1179` and `holdings.rs:110`
  (files changed since main, the branch point), `init.rs:606` (`git init -b main` for a new
  project; unaffected).
- **The session named `main`, 13 sites in 11 files.** `paths.rs:61` names the primary checkout
  `main` as a worker; `hook.rs:948` maps that name to the `owner` role; `launch.rs:596,1058`
  reserve it; `batch.rs:380`, `readers.rs:403`, `fleet.rs:78`, `land.rs:117`, `fanout.rs:770`,
  `batch_cut.rs:572`, `status.rs:838,1050` and `holdings.rs:107` compare a worker name to it.
  None of these may follow a base key.

Besides the bare literals, 152 string lines mention main in prose: 83 in `selftest.rs`, 17 in
`install.rs` (SURFACE notices, which never change), and 52 in refusals and notices ("contains
main", "merge main", "main moved to"). `docs/rules/roles.md` says `main` 36 times.

The one-function version: `base_branch(repo) -> String` beside `heartbeat_minutes` in
`handover.rs:250-262`, which already parses `.claude/air.json`, default `"main"`; the 15 branch
sites take it; the 13 session sites do not; `selftest` sets the key in one scenario. Gates whose
meaning changes:

| Gate | Today | With a base |
|---|---|---|
| close (`handover.rs:37-44`, `gate.rs:280`) | green at a commit containing the recorded `main_sha` and the bead's commits | the same of the base; `verify_runs.main_sha` (schema v19) becomes the base's sha, so the column's name lies or is renamed |
| batch-ready and landable (`batch.rs:234,264`) | landable means green at a head containing main | containing the base |
| cut (`batch_cut.rs:119,284,355`) | reset the lane to main, merge, name a conflict with main | the base |
| land (`land.rs:1005,1075-1100`) | the main checkout has main checked out; `commit-tree` with main as first parent; `merge --ff-only` | the main checkout must have the base checked out, or the landing moves to `update-ref`; "main moved to" becomes "<base> moved to" |
| status (`status.rs:1132,1179`, `holdings.rs:110`) | changed since main | since the base |
| roles text | "git merge main", "contains `main`" | the base, in 36 places |

Nothing is remade; the worktrees stay, and each merges the base instead of main. The size is
one bead's day plus a live-trial scenario. The risk is not the size but the overload: a
replacement that follows the word rather than the sense breaks the session-name sites, and the
only test that catches it is a fleet whose base is not `main`.

## 4. Who lands the feature branch onto main

The owner, by hand. Air lands only by fast-forward onto a commit whose tree has a green, and
re-verifies nothing (`land.rs:1075-1100`; design §6.3). Landing a feature branch onto a main
that has moved is a true merge whose tree no green covers, which is the state Air refuses today
("its green is not a green of what would land", `land.rs`, the `main-moved` refusal). For Air
to land it, something would have to verify the merge first: a second lane, or the owner
running `make verify` on the merge. With §2 the owner's `git push origin main:feature` and the
forge's own merge already do this, and the forge's queue, if it has one, verifies it. Air
recording the final landing would be a row for a move Air did not make, which the ledger rule
("only what git and beads cannot rebuild") excludes.

## 5. The do-less questions

- **Recorded failure removed:** none. No adopter has recorded a failure caused by landing onto
  main; the adopter's own entry lists this under "neither a problem today". The one recorded
  cost in the area is the opposite coupling, "Requiring it took every waiting branch out of the
  queue at every landing" (`status.rs:611`), which a base key would reproduce once per base.
- **Fact or judgement:** a fact (the branch name in a file). But the fact is already a git
  fact, and git expresses it (§2) without Air reading it.
- **Measurement first:** nothing in the ledger says how often a fleet wanted a different base,
  and no mechanism can measure a wish. The measurement is the ask: the number of adopters who
  have asked, which is one entry under "to explore", not a failure.
- **Removal condition, if ever built:** the key goes when no adopting repo has set it for a
  round, or when the harness or git carries the base for Air to read.
- **Smallest version:** no mechanism. One short section in `docs/rules/adopting-air.md` with
  the four git lines of §2 and the trunk-based warning. If an adopter later records a failure
  the git answer does not remove, the one-key shape of §3 is the next step, and a per-bead
  base (each queued item naming its own base, as every queue in §1 does) is never the step.

## 6. Is an explicit "ready for the lane" step wanted

No, for workers. A branch is batch-ready on the facts in `batch_ready_rule` (`status.rs:607`):
a commit main lacks that carries a `Bead:` trailer for a held bead, and a green precheck where
the repo sets one. The hold-back a worker might want already exists and is the trailer: a
commit without one is attributed to nothing and makes nothing batch-ready, so a worker who is
not done commits without the trailer and adds it on the commit that is. The close still passes,
because the gate asks for a green containing every trailered commit and the untrailered ones
are their ancestors (`gate.rs:15,64`). The roles do not say this; one sentence in the worker
section would, and it costs no mechanism.

The coordinator is the case with a recorded cost (every coordinator commit at the adopter cost
a batch and two notices), and that is air-m8u, `coordinator_autobatch`, a mark for the one
branch with no bead and so no trailer to withhold. The first half of the adopter's entry, "how
the lane is told", is already Air's: the lane receives `batch-ready: <worker> at <sha>` from
the channel (roles, the lane section). Before any further step, measure the thing the step
would fix: batches whose member committed again within a few minutes of the cut, which the
`batches` rows and `git log` give today.
