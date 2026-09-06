# 0006 — Changes proposed from the adopter's round, and the do-less pass over them

Status: ruled and built 2026-08-21 (owner: do A1-A4, A8, B1-B4, C1, C4, C6, C7, C9; D1 = `human`,
D2 = tmux yes, D3 = keep the deny; A5 replaced by the hand-over order fix: digest before the
final verify). Part 1 lists every change the record suggested, with its source. Part 1 lists every change the record suggests, with its source.
Part 2 passes each through the `do-less` skill and gives a verdict. Sources: the round record
(`../notes/rounds/2026-08-21-adopter/`), `../notes/air-backlog.md`,
`../research/guardrails-as-throttles.md`, and the adopter's own notes named in the round README.

## Part 1. Proposed changes (everything the record suggests)

### A. Correctness bugs in Air (things Air did wrong)

| # | Change | Source |
|---|---|---|
| A1 | `air release` reopened a closed bead: read `bd show` first; refuse on a closed bead, naming the rule; release only the ledger row (`reason closed`) when bd is already closed | backlog 14, |
| A2 | Timeout message asserted "nothing recorded" when the write had landed: say "timed out; bd state unknown, check `bd show <id>`"; decision value `timeout`, not `bd-refused`; write the ledger row only after a confirmed result | backlog 2, 15, |
| A3 | Claim rows survive `bd close` / `awaiting_review` → ~40 noise alerts: on a successful hand-over or close command, release the claim (reason `closed`/`handed-over`) or reconcile open claims against `bd show` status in `status` | backlog 4, 18; round log |
| A4 | Coordinator cannot release a peer's claim: `air release <bead> --worker <name> --reason reassigned` from the coordinator role | backlog 5 |
| A5 | Two verifies per hand-over: green at G carries to HEAD when `git diff --name-only G..HEAD` touches only repo-declared verify-irrelevant paths (`verify_ignores`); event says "carried over N commits (docs-only)" | backlog 1,; 3 of 5 hand-over refusals |
| A6 | `status` floods with one `handover-not-green` per handed-over bead: collapse to one line per worker with a count (falls out of A3 + A5) | backlog 18 |
| A8 | Channel noise, counted by the coordinator: ≈38 pushes, zero actionable. `handover-not-green` ×29 on beads already closed or in `awaiting_review` with green at the previous HEAD; `idle-with-claim` ×8 listing 11-13 handed-over beads every 20 min after the round ended; `gone-with-claim` ×1 on a closed bead. Three actionable pushes came through the same kinds (a worker stacking the next bead before `awaiting_review`). No subagent join/leave noise. Root causes are A3 and A5; fixing them removes the set. Rule: a kind stays pushed only while the coordinator acts on it | coordinator count, 2026-08-21 |
| A7 | Confirm (gone-with-claim on a fresh idle worker) is fixed after reinstall; drop | backlog 3 |

### B. Text that is wrong about the tools (remove or correct)

| # | Change | Source |
|---|---|---|
| B1 | Remove "assign with `assignee`" from roles.md and "the assignee is the suggestion" from the decomposition skill: in bd 1.2.x a pencilled assignee blocks every other worker's `--claim`; document bd's rule; `air claim` prints bd's refusal with who is assigned | backlog 17; three collisions, ~6 messages |
| B2 | Soften "coordinator runs this" in roles.md and the decomposition skill to "coordinator files and decides; the reading may be delegated with a file deliverable" | decomposition delegation, wave 1 found 3 epics done |
| B3 | adopting-air.md: "deny the verb, not the tool; patterns match command tokens, never substrings" |, ~16 prose-match hits |
| B4 | adopting-air.md: "cheap checks first; a gate that reports 10 minutes after the edit is a throttle" with the 4 s vs 10 min number | |
| B5 | adopting-air.md: coexistence default already there; add "retire only with a passed proof" examples from the round (cron deleted after channel proof) | round |

### C. Measurements to add (facts, no rules)

| # | Change | Source |
|---|---|---|
| C1 | Attention events record which kinds fired and for whom; store first-seen/cleared per (worker, kind) → time-to-unblock | backlog 11; counts gap |
| C2 | `record` fills `failing_step` (repo-declared stage marker, or stages run as separate kinds cheap-first); link reruns ("rerun of <id>"); sum rerun minutes per worker | backlog 12; 76 of 123 verify minutes red |
| C3 | Count `SendMessage` tool calls per worker (from PreToolUse `inputs.tool`); subscribe `SubagentStart` | backlog 13; relay cost invisible |
| C4 | Subscribe `PermissionDenied` and `PostToolUseFailure` (verify they exist): per worker, tool, command, refuser, time to next success | backlog 8; cargo-test denials, compound-bash refusals |
| C5 | Fleet-wide red as one fact: "N workers red at the same failing_step within M min" | backlog 9; research-label incident |
| C6 | Ready-queue depth and epics-awaiting-decomposition over time | backlog 16; queue went dry |
| C7 | Capture → bead ratio (promoted 1:1 / merged / dropped) as a standing number | 11 → 5 by hand |
| C8 | Review wait, S1/S2, per-session claim counts surfaced (`air metrics`) | backlog 10; metrics orphaned by `bd events` removal |
| C9 | Measurement spec rows: time-from-edit-to-red, reds-per-hand-over | |
| C10 | Verify whether `PermissionRequest` fires under auto mode (0 events all round) | counts |

### D. Role and policy changes (judgement-shaped; need the owner)

| # | Change | Source |
|---|---|---|
| D1 | Owner-decision beads not claimable by workers: one label or status means "awaiting the owner"; `air claim` refuses it naming the rule; settle `human` vs `owner` with the adopter | owner |
| D2 | `--tmux` launch from the coordinator (attachable pane; interactive rule intact) | coordinator request; owner undecided |
| D3 | Keep or drop the `bd create` deny for workers: data says 11 captures → 5 beads (dedup real); the adopter's "if you raise it, file it" contradicts it | backlog/audit "decide on round-one data" |
| D4 | Free-text `--reason` on `air release`, enum as suggestions | guard inventory |
| D5 | Stop conditions in roles.md as prose vs counters | boundary inventory UNCLEAR |
| D6 | `make lanes` promised-file tracking: an Air fact or not | boundary inventory UNCLEAR |
| D7 | the adopter's lease-guard PreToolUse hook (the cargo-test denier): keep or delete (the adopter's call; Air's view is delete, `air lease` covers it) | boundary inventory |

### E. Process lessons (how we build, not Air)

| # | Lesson | Source |
|---|---|---|
| E1 | Cut beads so no shared doc is touched by two at once; the same file both ways was the only real overlap shape | 4 peer warnings, intake.md ×2 |
| E2 | Cheap gates first; doc rules at the edit or first in verify | 4 s vs 10 min |
| E3 | Three workers + coordinator + verify on one machine saturates it (load 22-25); bd timeouts were load, not contention; size the fleet to the machine | 4 claim timeouts |
| E4 | Read before decomposing; three epics were already done on main | wave 1 |
| E5 | A label can be a schema when a fitness check parses it; check the documented set first | research-label incident |
| E6 | Run verify three times at one commit before adopting a gate; flakes become correctness | |
| E7 | Queue depth over time tells you when the coordinator becomes the bottleneck | queue dry at 4 |

### F. Roadmap items confirmed by the round (unchanged)

`air init` (greenfield, bd gate first, `record verify` as first proof, scan for publish
targets, quiet hooks), `air land` (ledger claim + merge range, never assignee; `landed-but-open`),
`air next` only after a measured stale-pick rate, dogfooding.

## Part 2. The do-less pass

Each item: which recorded failure; fact or judgement; measurement or gate; silent; removal
condition; smallest version. Verdicts: **do** (correctness or a measurement with an incident),
**do, smaller** (a reduced form), **owner** (judgement-shaped), **drop**, **defer** (no incident
yet or depends on data).

| # | Failure cited | Fact / judgement | Verdict | Smallest version and removal condition |
|---|---|---|---|---|
| A1 | reopened a closed bead | fact (bd state) | **do** | one `bd show` before `-s open`; no removal (it is a bug fix) |
| A2 | lied about state | fact | **do** | wording + `timeout` decision; ledger row only after confirmed result; remove the internal retry if events show zero timeouts in a round |
| A3 | ~40 noise alerts | fact | **do, smaller** | reconcile in `status`/poll: a claim whose bead `bd show`s as closed is treated as released (no hook watching, no new write path); remove if `air land` later owns closure |
| A4 | gone worker's bead stuck | fact | **do** | `--worker` flag, coordinator role only, event names both |
| A5 | 2 verifies per hand-over | fact (git diff) | **do** | derive, never assert; `verify_ignores` in air.json; event says carried-over; measure first for one round is optional since the incident count (3 of 5) is already in hand |
| A6 | screen flood | fact | **do** (via A3/A5) | nothing separate |
| A7 | none new | | **do** (confirm, drop) | |
| A8 | ≈38 noise pushes, 0 actionable | fact | **do, via A3 + A5** | a claim on a closed or `awaiting_review` bead is not "held": reconcile against bd status before any condition; green carries over verify-irrelevant commits. Then re-count; any kind still marked noise next round is removed outright. `idle-with-claim` must not count handed-over beads as held |
| B1 | 3 collisions | wrong text | **do** | delete the lines; one sentence of bd fact; no new rule |
| B2 | delegation worked | judgement | **do, smaller** | delete "coordinator runs this"; say nothing about who reads (the model decides); the skill keeps file + decide with the coordinator |
| B3 | 16 prose-match hits | fact about patterns | **do** | one sentence in adopting-air |
| B4 | 4 s vs 10 min | fact | **do** | one sentence with the number |
| B5 | none | | **drop** | coexistence is already stated; examples are prose |
| C1 | kinds not recorded | measurement | **do** | log kinds; cleared_at per (worker, kind); remove never (it is the record) |
| C2 | 76 red minutes unexplained | measurement | **do, smaller** | `failing_step` from a repo-declared marker only if the repo provides one; rerun linking is derivable at query time (same HEAD, same command, previous red) so do not store it; sum at query time |
| C3 | relay cost invisible | measurement | **do, smaller** | query `inputs.tool == "SendMessage"` from existing lines (no new hook); `SubagentStart` defer until a count is wanted |
| C4 | foreign denials invisible | measurement | **do** | after verifying the two events exist in current docs; record only; remove never |
| C5 | fleet red read as N bugs | fact | **defer** | derivable at query time from `verify_runs`; add to `status` only if a second incident occurs |
| C6 | queue went dry | measurement | **do, smaller** | one number per `status` tick (`bd ready` count) on the existing bd call; no second series |
| C7 | 11 → 5 by hand | measurement | **do** | computed from `captures` rows (promoted with/without a note) at query time; no new column |
| C8 | metrics dark | measurement | **defer** to plan 0005 | data exists; surface when a round asks for the number |
| C9 | | spec rows | **do** | two rows in the measurement spec |
| C10 | 0 events | unknown | **do** | one manual check in the next round |
| D1 | owner | fact about the bead | **owner** | recommend: one status or label; `air claim` refuses with the reason; smallest: reuse the owner-queue capture and a `human` label bd already understands |
| D2 | owner | | **owner** | recommend yes: rule intact, coordinator autonomy gained |
| D3 | owner | | **owner** | data favours keeping the deny (dedup real); recommend keep for one more round, recount |
| D4 | guard inventory, no incident | | **defer** | no worker fought it; revisit if one does |
| D5 | UNCLEAR | | **owner** | recommend counters where a fact exists, delete the rest |
| D6 | UNCLEAR, no incident | | **drop** | holdings already derive files touched |
| D7 | The adopter's | | **The adopter** | Air's view: delete; `air lease` covers it |
| E1-E7 | | process | **record** in adopting-air §2 as lessons, one line each; no mechanism |
| F | | | **unchanged** | |

### What the pass removes from the proposal list

B5, C5 (for now), C8 (to roadmap), D4, D6. And from C2 and C3 the stored forms: derive at
query time instead.

### What the pass would build, in order

1. A8 first: cut every pushed kind the coordinator called noise. Then A1, A2, A3 (smaller),
   A4: the release/claim/timeout correctness set. One slice, one test
   each, no new rules.
2. A5 (+A6): carry-forward green over verify-irrelevant paths. The single largest time cost in
   the round that Air caused.
3. B1, B2 (smaller), B3, B4: text corrections; delete more than add.
4. C1, C4, C6 (smaller), C7, C9, C10: measurements.
5. Owner rulings D1, D2, D3, D5; then whatever they say.

Everything else waits for the next round's record.
