# 0009 — Fleet system design: a verification lane that lands, and nobody in the main checkout

Status: **draft, 2026-09-14; revised 2026-09-25** against an adopter's fleet protocol (§11).
Written with the `system-design` skill as its first application. Built so far: role read from
`AIR_ROLE` (decisions, 2026-09-14, `cmd::role_from`); on 2026-09-25 the batch-ready change and
the nudge fix in §5, and the roles text carrying the protocol (step 5, except the parts that
wait for the launcher). Section 10 lists what the
owner rules on; sections 1 to 9 are the recommendation and its evidence.

**Where the protocol lives** (owner, 2026-09-25): the merge-queue protocol — each role's
sequence, the batch-ready rule, the close window, conflict handling, what a lane does per cut —
is Air's, and ships in `.air/roles.md` and in Air's commands. An adopting repo keeps only what
is truly its own: its verify and precheck commands, its worktree setup, its resources and
leases, its test-state reset. `roles.md`'s "how a finished bead is handed on is the repo's own
flow" (air-8zu, air-97z) is reversed by this.

Answers:
- An adopter's round of 2026-09-05 to 2026-09-07 (their restart note of 2026-09-07): a batch
  landed on the `landable` fact before its members had closed and stranded six finished beads;
  a batch killed mid-verify read as a red; the coordinator committing in the main checkout;
  the coordinator filing 56 process beads in one night; a lane skipping its dry-merge once and
  letting typing order decide which member "conflicted".
- Air's own round of 2026-09-06 (`docs/notes/rounds/2026-09-06-air/2026-09-06-round-log.md`
  §15, §22): three green branches at once, and landing the first invalidated the other two, so
  "one landing per round of merges is the actual throughput"; a version bump's `make verify`
  regenerated `Cargo.lock` in the main checkout, dirtied it, and moved main under a worker's
  green.
- Air's 2026-08-22 round (decisions, air-3pz): two green hand-overs sat because nobody was
  allowed to land; and air-29a: a worker landed from a worktree by pointing `--repo` at main.
- The owner's ruling of 2026-09-14 (`docs/decisions.md`, same date): verification lane first-class,
  every role in a worktree, the coordinator's changes go through the lane.

Numbers it is sized against (adopter ledger copy, `verify_runs`, read 2026-09-14 with
`sqlite3`; the binary that wrote them was air 0.2.19 at their checkout):

| Day | verify runs | green | mean s | max s |
|---|---|---|---|---|
| 2026-08-21 | 46 | 21 | 199 | 683 |
| 2026-08-30 | 251 | 164 | 350 | 1801 |
| 2026-09-06 | 50 | 24 | 784 | 1748 |
| 2026-09-07 | 29 | 26 | 1131 | 2052 |

From 2026-09-05 every recorded verify was the lane's (`w4`): 83 runs, 54 green, 21.1 hours of
verify on one machine. `landings` on 2026-09-06: 11 landed, 9 landed-refuted, 12 refused.
`failing_step` is empty on all 142 reds since 2026-08-30, so the ledger cannot say why a
batch went red; the lane's log can. (Air, not the adopter: a defect to file, §10.)

## 1. Actors and ownership

| Store | Writer (exactly one) | Readers | Second writer stopped or detected by |
|---|---|---|---|
| `main` | the verification lane, by `air land` (fast-forward to a verified integration commit) | every worktree via `git merge main` | `air land` refused outside the lane's session (today: outside the coordinator's; air-29a checks the caller's cwd, not `--repo`); `git push` denied to all; the main checkout has no session |
| `worktree-wN` branch and files | worker N | the lane merges it at a listed sha | one session per worktree; PreToolUse path fence (air-8gj) |
| `worktree-lane` branch | the lane (throwaway integration commits) | nobody needs to | as above |
| `worktree-coord` branch | the coordinator | the lane merges it like a worker's | as above; the coordinator's deny list keeps `bd create` and loses `air land` |
| task store | coordinator creates, prioritises, edges; each worker claims and closes its own | all | `bd create` denied to workers and the lane; claim is `air claim`; close refused without green |
| ledger | `air` commands and hooks | `air status` | append-only rows |
| sessions | the launchers | `tmux attach`, `SendMessage` | named `<project>-<name>`; not another project's |
| the queue to main | nobody maintains it: it is the batch-ready set, derived | the lane | it cannot drift because it is not stored; its order is §4's rule |
| the main checkout's files | nobody | `air status` reads `.air/` there | a landing that finds it dirty refuses, naming the file; with no session there, the only way it gets dirty is a program run there by hand |

Today's departures from this table, from `launch.rs` and `land.rs` read 2026-09-14: the
coordinator is a session in the main checkout and may commit there (notice
`coordinator-may-commit`); `air land` is the coordinator's; the lane does not land.

## 2. Flows

```mermaid
sequenceDiagram
  participant W as worker wN (worktree)
  participant L as verification lane (worktree)
  participant M as main
  participant C as coordinator (worktree)
  W->>W: air claim; commits; digest; git merge main
  W-->>L: batch-ready (fact in air status) + one message
  L->>L: dry-merge members pairwise (git merge-tree)
  L->>L: cut batch = main + members at listed shas
  L->>L: air record verify -- make verify
  alt green
    L->>M: air land (fast-forward main to the integration commit)
    L-->>W: landed: your beads a,b at <sha>; close on it
    L-->>C: landed: beads; members
    W->>W: bd close (green at a commit containing main and the trailer commits)
    W->>W: git merge main; next bead
  else red
    L-->>W: red at <sha>, step X, you were a member
    L-->>C: red by member
    L->>L: re-cut without the member the step names, or bisect
  else conflict
    L-->>W: dropped: conflicts with wM at <path>; resolve in your worktree
  end
  C->>C: triage captures; decompose; prioritise; commit prose on its own branch
  C-->>L: batch-ready like anyone
```

| Hop | Actor | Acts on | Trigger | If the trigger never comes |
|---|---|---|---|---|
| worker → ready | worker | `batch-ready` line | its own `git merge main` after the digest commit | the fact is in `air status`; the lane polls it before each cut, so a forgotten message costs one cut's latency, not the bead |
| lane cuts | lane | batch-ready set | its own loop: after each landing or red, and on a poll interval while idle | nothing lands; `air status` shows the set growing (a `batch-waiting` count is the measurement, §5) |
| lane lands | lane | its own green at the integration commit | `air record verify` exit 0 | a green with no landing is `landable` (existing condition) |
| worker closes | worker | landed message, or `air handover` naming the green | message | `landed-not-closed` fires (existing condition) |
| worker merges main | worker | main moved | for its own staleness and to resolve a conflict the lane named; never to re-enter the queue (§11) | nothing: the lane merges main forward onto each member at its listed sha, so a branch stays batch-ready while main moves; a member that conflicts with main is dropped and named like a pairwise conflict |
| coordinator lands its prose | coordinator | its own branch | same as a worker | same as a worker |

## 3. Invariants

| Invariant | Who could break it | Detected by | Refused / reported / measured |
|---|---|---|---|
| Every commit on `main` has a green `verify_runs` row at its sha or at a commit whose tree is identical | anyone with a shell in the main checkout; `air land --despite-inflight` | `air audit` walk of `main` against `verify_runs` | refused at `air land` (existing, air-odv); a bare `git commit` on main is only detectable, and is why no session lives there |
| `main` moves only by fast-forward from the lane's session | coordinator, owner | landing rows name the lane; a merge commit on main not in `landings` | refused: `air land` outside the lane's cwd; measured: the audit line |
| A worktree contains one branch and only its own commits plus merges of `main` | a worker merging a peer's branch | `git log main..worktree-wN` shows a foreign trailer | reported by `air status` (a bead on two branches is already refused at `air land`, air-09b) |
| A bead closes only on a green at a commit containing `main` and every commit carrying its trailer | worker | the close gate | refused (existing, `AIR_ENFORCE=1`) |
| A close is valid after the landing as well as before it | Air itself (air-9ij: `bead_commits` empty once landed) | the probe for air-9ij | refused wrongly if it regresses; §10 asks to confirm the fix covers the adopter's stranding of 2026-09-07 before the "wait for every close" protocol is dropped |
| The lane holds no bead while a batch is cut | lane | `claims` for the lane's worker name | measured; `air status` line |
| No session's cwd is the main checkout | owner, by hand | `sessions` rows' cwd | measured; `air status` warns |

## 4. Failure catalogue coverage

Walk of `.claude/skills/system-design/references/failure-catalogue.md` (rows by id). "Existing"
means Air already does it and the design keeps it; "this design" means §5 adds or changes it.

| Class | Coverage | How |
|---|---|---|
| A1 session in the main checkout | prevented | this design: no session there; coordinator's prose is a branch through the lane |
| A2 landing/close race | prevented | invariant "close valid after landing"; probe in §5; the adopter's wait-for-closes protocol not adopted |
| A3 program commits on a worker's branch | accepted, named | Air commits nothing on branches; the adopter's re-cache scripts are theirs; a lane that regenerates caches is a later bead |
| A4 two claim stores | existing | ledger claim reconciled to bd on retry (`claim-records-the-resolved-id`) |
| A5 assignee as claim | existing | roles.md; `air claim` never sets it |
| A6 peer acts in your worktree | existing | path fence (air-8gj) |
| B1 relayed sha | prevented | the lane reads batch-ready from `air status`, never from a message; a worker's message carries the sha as a courtesy |
| B2 readiness is a shape | this design (built 2026-09-25) | workers do not verify under the lane; a red batch names the member. A worker's precheck is a recorded run, `air record precheck -- <cmd>` (`Kind::Precheck`), never read as a verify green. Where `.claude/air.json` sets `"precheck": true`, `batch_ready_rule` wants a green one at the head (`no-precheck`), replacing the adopter's log file and "checked at <sha>" message (§11) and the relayed sha (B1) with a row keyed by sha. A running precheck is in flight like a verify: not idle, and it does not hold a landing |
| B3 stale trailer as verdict | existing | verdicts are ledger rows keyed by sha |
| B4 snapshot as clearance | existing (Air main) | `close-asks-the-recorded-main` |
| B5 number from the wrong binary | existing | `project-diligence`; this doc names its binary |
| B6 derived read as observed | existing | `do-less` raw-record check |
| C1 message-only handoff | existing | journal, digests, bead comments |
| C2 remedies with no audience | this design | roles text rewritten per role; Stop hook text lane-neutral (Air main, air-avj). `handover-not-green` counts only refused closes since air-zqmi (2026-09-06, after the adopter's 0.2.19), so a member closing on the lane's green no longer raises it. **Not yet**: the Stop nudge offers ready beads to the lane, which is `AIR_ROLE=worker` today (`gate.rs` `stop_nudge`); §5 |
| C3 silence as instruction | existing | roles.md worker loop |
| C4 named, not claimed | existing | roles.md; notes on the bead |
| C5 worker cannot tell it has something to do | this design | the lane's landed message carries three lists; `air handover` names the batch (air-hpp8) |
| C6 coordinator inside a long read | existing | background agents with file deliverables; landing leaves the coordinator |
| D1 landing invalidates greens | prevented | one integration commit, one verify, one fast-forward |
| D2 killed run as red | existing | `killed-is-no-verdict`; the lane re-cuts |
| D3 conflict by order or pipe | prevented | pairwise `merge-tree` before the cut; drop recorded |
| D4 test state leaks between cuts | accepted, repo's | the lane's own `make` target resets; Air reads nothing about databases |
| D5 fixtures stranded | accepted, repo's | fixtures are the repo's; a throwaway worktree is the repo's choice |
| D6 verify dirties the tree | prevented for main | no session in the main checkout; on a worker's branch it is the worker's commit |
| D7 scope narrower in the lane | this design | the lane's launcher sets the scope env, not a habit |
| D8 red with no failing step | this design | recorder writes the step |
| D9 two wrappers, two pids | accepted, repo's | one recorder is `air record`; the adopter's wrapper is theirs |
| D10 re-verifying for nothing | prevented | batching |
| E1 kill by pattern | existing | `task-by-file` |
| E2 sessions die, no restart | accepted | §10 ruling |
| E3 settings replaced | existing | `env-on-the-process`, merged `--settings` |
| E4 blocking prompt | existing | `AskUserQuestion` denied; stdin `/dev/null` on detached start |
| E5 too many sessions | this design | five sessions, one verify at a time, stated in §7 |
| E6 orphaned watchers | accepted | not seen in Air's fleet; the adopter's watchers are theirs |
| E7 busy read as idle | this design | `air status` says "no run recorded" for a worker with no run, never "idle" |
| E8 cannot stop own process | accepted, harness | recorded; the classifier is the harness's |
| E9 sessions indistinguishable | existing | `<project>-<name>` |
| E10 rewriter changes a measurement | accepted, owner's tooling | recorded |
| F1 guard on a string | existing | launcher deny of binaries; `--reason-file` |
| F2 whole call discarded | accepted, harness | recorded; Air's hook denies only Edit/Write paths and the close |
| F3 tool-level deny swallows reads | existing | Air's list denies verbs (`bd create`, `git push`); the repo's `worker_deny` is the repo's |
| F4 enforcement editable by the enforced | existing | deny list arrives with the binary |
| F5 four copies, one stale | this design | roles text is the one copy; the lane section replaces the verification-lane section |
| F6 soft deny nobody answers | accepted, owner's settings | recorded |
| F7 long argument refused | existing | `--reason-file`, `--file` |
| F8 killed hook as clean pass | existing rule | decisions 2026-09-06 |
| F9 permission missing where the role now runs | this design | §6 matrix; step 4's probe launches the coordinator in a worktree and runs each of its commands |
| G1 queue fills with findings | accepted, coordinator's judgement | roles.md; the adopter's "does it change what anyone does tomorrow" rule is quoted there, not enforced |
| G2 store changes under the tooling | existing | `air doctor` pin |
| G3 slow store on the hot path | existing | one process per command; never from a hook |
| G4 two acceptance texts | accepted, bd's | recorded |
| H1 knowledge lost with a session | existing | journals committed; the lane's round note tracked |
| H2 two date conventions | existing | ledger timestamps are UTC |
| H3 sweeps only accidents catch | existing rule | CLAUDE.md round-end duty |
| H4 record cannot say why | this design | D8 plus `red-run-output-kept` |

Known gaps: E2 (respawn), E6/E8 (process hygiene
inside a worktree), and the in-flight refusal's blind spot: `air land` sees only verifies
recorded through `air record`, so a landing still moves files under an unrecorded program in the
main checkout. With no session there this shrinks to programs run by hand; an adopter checks for
them by process cwd before every landing (§11), which is repo tooling Air does not take on. Each
is measured or ruled in §10 rather than left to be found.

## 5. Mechanism table

| Mechanism | Kept / added / deleted | Recorded failure | Kind | Probe | Removal condition | Adopter-visible |
|---|---|---|---|---|---|---|
| `air land` allowed in the lane's session, refused in the coordinator's | changed | adopter 2026-09-07 (coordinator lands, in main checkout); round log §15 | refusal | selftest: land from `AIR_ROLE=coordinator` refuses; from `AIR_ROLE=lane` proceeds; red seen when written | when landing needs no session (a program the lane's loop calls) | yes: notice |
| `air lane` launcher (or `air worker <name> --role lane`) | added | adopter keepalive prompt hard-codes the lane's role in prose | fact (env `AIR_ROLE=lane`, deny list for the role) | `--print` shows the argv; selftest | when the harness provides per-role launch | yes: notice |
| `air coordinator` starts in a worktree, with a tmux session | changed | round log §22; adopter 2026-09-07 §3 | fact | `--print`; selftest that the cwd is not the main checkout | never, while more than one session exists | yes: notice |
| Coordinator deny list: loses `air land`, keeps `bd create`, `air triage`, launching | changed | as above | refusal (deny rules hold in every mode, harness docs 2026-09-05) | selftest over the constant | with the launcher | yes: notice |
| Roles text: "role is the env", lane section rewritten as the verification lane | changed | roles.md "Role is the checkout" would call the coordinator a worker | prose | none | with the launcher | yes: notice |
| Pairwise `git merge-tree` before a cut | added | adopter 2026-09-07 §3 | fact (prints the conflicting pair and path) | selftest with two branches touching one line | when `git merge` itself reports the pair | yes: notice |
| `batch-waiting` count and age in `air status` | added | measurement first (do-less q3): how long branches wait for the lane | measurement | selftest | when a round shows median wait under one verify duration | yes: notice |
| Red-batch policy: drop the member the failing step names, else bisect | added | adopter 2026-09-06: 9 landed-refuted, 12 refused in one day | fact + the lane's judgement | selftest for the pure ordering | when reds per batch fall below one in ten for a round | yes: notice |
| Ledger rows for "dropped from batch: conflict with X at sha" and "retried once at sha" | added | said in prose, recorded nowhere (research §9, queue state) | fact | selftest | never; they are what makes a second red a second red | yes: notice |
| `failing_step` recorded on every red | fixed | 142 reds with no step in the adopter's ledger | fact | selftest: a red run has a step | never | yes: notice |
| Batch-ready no longer requires the head to contain main; the lane merges main forward at the cut | changed | an adopter (§11): under "head contains main", every landing takes every waiting branch out of the queue until its worker re-merges, and their workers were told to merge "for your own close and for staleness, not for the cut" | fact (`batch_ready_rule`, `status.rs`) | selftest probe and `status_lists_batch_ready_branches_as_a_fact` (built 2026-09-25); the lane dropping a member that conflicts with main is step 3's `merge-tree` | never, while the lane merges main itself | yes: notice `batch-ready-behind-main` |
| `air record precheck`; batch-ready wants a green precheck at the head where `"precheck": true` | added | an adopter, 2026-09-05..07: a worker cut before its check finished, "checked" relayed for a check still running, a worker nudged as idle 400 s into an unrecorded precheck, and a log trailer naming a head two commits back | fact (`Kind::Precheck`; `batch_ready_rule` field `precheck_green_at_head`) | selftest probes for the rule and for "a precheck green is never a verify green", both seen red; `a_declared_precheck_gates_batch_ready_and_is_not_a_verify` (built 2026-09-25) | the key: when a round under it has no batch red that a member's precheck would have caught, the precheck only delays the cut | yes: notice `precheck-is-a-run` |
| Stop nudge skips the lane | fixed | an adopter (§11): the Stop hook offers the lane ready beads; the lane claims none | fact (the Stop hook reads `verify_lane` from `.claude/air.json`; built 2026-09-25) | `stop_nudge_skips_the_verification_lane`, seen red with the lane check removed | when the lane has its own launcher and role (step 2) | yes: notice |
| "Wait for every close before landing" (adopter protocol) | not adopted | air-9ij fixed the cause | | | | |
| `worker-keepalive.sh` respawn loop | not adopted here | five deaths 2026-08-30 | | | §10 | |

## 6. Permissions matrix

| Role | May | May not | Means | Citation |
|---|---|---|---|---|
| worker | edit its worktree; `air claim`; `bd close` with green; `SendMessage`; `air capture` | `air land`, `air close`, `git push`, `bd create`, `bd sync`, raw `bd update --claim`, nested `claude`, leaving the worktree, `AskUserQuestion` | `--disallowed-tools` (deny in every mode) + PreToolUse fence | https://code.claude.com/docs/en/permission-modes "Available modes", accessed 2026-09-05 (decisions, air-4t1); `launch.rs` read 2026-09-14 |
| verification lane | everything a worker may; `air land`; `air record verify` on a batch | `bd create`; claiming while a batch is cut (measured, not refused); resolving a conflict | deny list minus `air land`; the cwd check in `land.rs` | as above |
| coordinator | `bd create/update/comment`, `air triage`, `air close` (for landed beads), launching workers and the lane, `SendMessage`, editing its worktree | `air land`, `git push`, editing the main checkout, editing another worktree | deny list; fence | as above |
| owner | all | | | |

Open: whether the coordinator keeps `air close`. It is the recovery tool for a bead whose worker
is gone; it is also how the adopter recovered the six stranded beads. Keep, with its use
counted (it already is: the `close` command's ledger row).

## 7. Capacity

Sessions: 5. Concurrent verifies: 1 (the lane's). Throughput at the adopter's 2026-09-07 mean
(1131 s): about three batches an hour; at Air's own (`make verify` here, roughly 5 minutes on
2026-09-06 per the round log): about ten. When the batch-ready set grows faster than the lane
cuts, the set queues in `air status` and `batch-waiting` says how long; nothing refuses. A P0
member is cut alone (the adopter's rule of 2026-08-30, kept as the lane's judgement, not a
mechanism).

## 8. Program versus judgement

| Step | Program | Judgement |
|---|---|---|
| which branches are ready | `air status` (batch-ready rule, `status.rs:517`) | none |
| member order | oldest batch-ready first | the lane may pull a P0 forward |
| conflict detection | `git merge-tree --write-tree` pairwise | none |
| conflict resolution | never the lane | the member's worker, in its worktree |
| the cut | `git merge` of listed shas on a throwaway branch | none |
| verify | `air record verify -- make verify` | none |
| green → land | `air land` (fast-forward; tree identical to the verified one) | none |
| red → next cut | drop the member the failing step names, if the step names a path a member changed; else halve | the lane reads the log when the step names nothing |
| who closes | the worker, on the lane's green | none |
| what the coordinator files | | the coordinator's, with the adopter's 2026-09-07 rule: a capture becomes a bead only if somebody edits a file because of it |

## 9. Migration

| Step | Runnable after | Probe | Prose deleted |
|---|---|---|---|
| 1. Record `failing_step` on every red; add `batch-waiting` to `air status` | yes | selftest | none |
| 2. `AIR_ROLE=lane` launcher; `air land` bound to it; coordinator loses `air land` | yes, with the coordinator still in the main checkout | selftest over the deny constants and the cwd check | roles.md "Landing stays the coordinator's" |
| 3. Pairwise `merge-tree` in the lane's cut (`air batch cut`, or documented git) | yes | selftest with a conflicting pair | adopter-style "dry-merge first" prose |
| 4. Coordinator in a worktree with a tmux session; role is the env; main checkout has no session | yes | `--print`; selftest | roles.md "Role is the checkout"; notice `coordinator-may-commit` superseded |
| 5. Roles text rewritten: the whole protocol per role (worker, lane, coordinator), including the §11 facts; the repo's commands come from `.claude/air.json` keys, not from its CLAUDE.md | yes | none (prose) | the verification-lane section; this repo's CLAUDE.md "This repo's work flow"; an adopter's protocol file, which it deletes itself once upgraded |
| 6. One round on Air itself; then the adopter | | the ledger: batches, reds, waits, strandings (zero) | |

Each step is a bead; steps 1 to 3 do not depend on 4. All of it is one release row at round end.

## 10. Owner rulings needed

| Question | Recommendation | Why |
|---|---|---|
| Does the lane land, or does it verify and the coordinator lands? | the lane lands | landing is the coordinator's largest main-checkout write; §15 shows landing order is the throughput, and the lane already knows the order |
| Is the coordinator in a worktree? | yes, with a tmux session | §22 and the adopter's restart note; ruled 2026-09-14 |
| Does the coordinator keep `air close`? | yes, counted | recovery tool; the adopter's stranding was recovered with it |
| Drop the adopter's "wait for every close before landing"? | **settled 2026-09-25: yes** | the probe exists and is green: `a_bead_already_in_main_closes_on_its_landing` (`crates/cli/tests/claim_cli.rs`, air-9ij limb 1), the adopter's stranding case exactly; their close-window rule and its checks were removed the same day |
| Does Air own session respawn (the adopter's keepalive)? | not in this design; separate check-resources pass | the harness gives no exit signal; the adopter's loop works; deciding it here widens the change |
| Red-batch policy: drop-by-step, or always bisect? | drop-by-step, bisect when the step names nothing | at n≤3 a bisect costs at most two extra verifies; drop-by-step usually costs one |
| Name: "verification lane", session `lane`? | yes, ruled 2026-09-14: the fleet is always described as three workers, a verification lane, and the coordinator | `air status` already says "lane" |
| Record a worker's precheck as a run kind, and have the batch-ready rule read it where the repo declares one? | **built 2026-09-25**: recorded always, read by the rule only where the repo sets `"precheck": true`, so the repo decides from its own reds | an adopter already gates its lane on a precheck, from a log file and a message (§11); a ledger row replaces both. The log is also wrong in a way a row cannot be: a hand-over that died before its precheck left the PREVIOUS run's green trailer in place, naming a head two commits behind (their restart note §5b, 2026-09-07) |
| Is an unclear acceptance a reason to ask the owner? | no: a writing defect the coordinator rewrites; `owner` is for a decision or an action only the owner has | the owner ruled this at an adopter; roles.md still lists "an ambiguous acceptance" |
| Is the lane a program with a session watching it, or a session running a program? | a program the session calls: `air batch cut` (readiness, `merge-tree` pre-check, the cut, the record) and `air batch next` (the split after a red); the session reads logs, makes the flake call, and talks to workers | research §9: everything but reading the log bors did without a model; a session that runs the loop by hand skipped its dry-merge once at the adopter and let typing order decide a conflict |

## 11. Reconciled against an adopter's fleet protocol (2026-09-25)

An adopter wrote its multi-agent rules into one file "meant to feed Air's protocol later",
holding only what Air's roles text did not already say. Copy:
`private/adopter-corpus/<adopter>/2026-09-25/fleet-protocol.md` (the file was uncommitted in
their checkout; they run air 0.2.19). Each rule falls into one of four places.

**Into Air's protocol** (roles text at step 5, or a mechanism in §5):
- A worker merges main for its own staleness and to resolve a named conflict, not to stay in
  the queue; the lane merges main forward at the cut. Adopted as the batch-ready change (§5).
- Once a sha is offered, commit forward and never amend: an amend forks beside what the lane
  holds, and the lane can only drop it. A fact about the cut; Air's close gate already refuses a
  bead with a trailer commit after the cut, naming it.
- Conflict resolution is the member's worker's, in its worktree, and a branch that does not
  merge cleanly is not finished. Two conflict facts go with it: both sides adding the same item
  parses and is wrong, and both sides adding to a count or list merges as text while the claims
  contradict. Check a merge by `git ls-files -u` and conflict markers, never by piped output.
- The lane holds no bead while a batch is cut (already §3). The adopter is stricter — none
  between batches either — and that stays the lane's call.
- The coordinator does not commit to main. Structural here: it has no session in the main
  checkout (§1).
- A timeout is not a verdict on a write: read the state, re-issue only what the tool said it did
  not write. A check whose negative has two meanings is not a gate. These are rules for Air's
  own code (the `anti-brittleness` skill), not role text.

**Stays the repo's**: worktree setup (keys, env files, per-worktree databases), the `runtime`
lease's resources, build concurrency limits, the test-state reset before each cut (D4), the
precheck command itself, and regenerated caches taken wholesale on conflict.

**Superseded by this design**: "stash, never commit, when `air land` refuses a dirty tree" (the
refusal went with air-odv, and no session is in the main checkout); the close window "close
every bead before main moves again" (air-9ij; §10 asks for the probe first); the lane taking
shas from "checked at <sha>" messages (B1: it reads `air status`; the precheck ruling in §10
replaces the message with a row); the list of Air advice that is wrong under a lane (three
fixed by air-avj, `handover-not-green` by air-zqmi, the nudge in §5, the in-flight gap in §4).

**The adopter's restart note of 2026-09-07** (copy in the 2026-09-14 corpus) was reviewed the
same day and deleted from their repo: its round state is eighteen days stale, and each of its
five protocol changes is already here — wait-for-closes (air-9ij, §10), a process-level
tree-readers check (§4 gap), the coordinator through the pipeline (§1), dry-merge before the
cut (§5, step 3), and the three-list announcement (C5). Its filing rule is §8's.

**Not taken**: dispatch advice to the coordinator (judgement, and roles.md already says long
reads go to background agents); "never remove a worktree because its branch merged" (Air makes
and reuses the worktree; no recorded failure here).

## Sources

- `docs/decisions.md` 2026-09-14, 2026-09-05 (air-4t1, air-80x), 2026-08-22 (air-3pz).
- `docs/notes/rounds/2026-09-06-air/2026-09-06-round-log.md` §15, §22.
- `docs/rules/roles.md`, "Verification lane" and "Coordinator", read 2026-09-14.
- `crates/cli/src/cmd/{launch,land,tmux,status}.rs`, read 2026-09-14.
- Adopter corpus, `private/adopter-corpus/<adopter>/2026-09-14/` (README there; restart
  note §2, §3, §5, §7; ledger queries in this doc's header).
- `docs/research/merge-queues-prior-art.md` (2026-09-14).
- An adopter's fleet protocol, copied 2026-09-25 to `private/adopter-corpus/` (§11).
- `crates/cli/src/cmd/status.rs` (`batch_ready_rule`, the `handover-not-green` red filter),
  `crates/hooks/src/gate.rs` (`stop_nudge`, air-avj), `crates/cli/src/cmd/mod.rs` (`role_from`),
  read 2026-09-25 at `27567b2`.
- `claude --help`, 2.1.272, 2026-09-14: `--tmux` still requires `--worktree`; Air's detached
  start stays.
