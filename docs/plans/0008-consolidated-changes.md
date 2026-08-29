# 0008 — The consolidated change list, ruled

**What this is**: every change the 2026-08-24/25 surface audit, the harness-and-orchestrator
landscape, and adopter's 2026-08-21/23 round logs proposed, in one list, with the owner's
ruling and the path for each. Filed 2026-08-29.

**Sources**: [`0007-surface-audit.md`](0007-surface-audit.md) ·
[`../research/harness-and-orchestrator-landscape.md`](../research/harness-and-orchestrator-landscape.md) ·
`~/projects/adopter/docs/log.d/2026-08-2{1,2,3}-*.md` (216 files, read 2026-08-29) ·
alpha's `air-okc` findings · [`../notes/air-backlog.md`](../notes/air-backlog.md).

**Rulings**: [`../decisions.md`](../decisions.md), 2026-08-29.

Two claims from adopter were checked before being carried and did **not** survive, which is
the cross-project rule working:

- "The Stop-hook nag lists the raw `bd ready`, so an `owner` bead still appears" — fixed already
  by air-ouw. `ready_cache::confirm` filters `owner`, and the cache is the gate, never the
  answer. Their own note said it was not verified empirically.
- ad-mhb8, "the hand-over gate names a probable cause as though it were a finding" — it does
  not. `gate.rs:73` says "no green verify recorded at HEAD <sha> (last green: <sha>)" with the
  fixing command. Their second-agent had already corrected their coordinator on this.

---

## 1. Delete or shrink

**1. `air lease` — KEEP.** Zero rows here; used all round in adopter, where a worker read
`air lease status`, saw `runtime` held by a peer, and took different work. Their bug is two
stores that disagree (`air lease take` vs `scripts/lease.sh`), so `make api` was denied naming
the command just run. **Path**: the adoption procedure gains a step that finds and collapses a
second lease store; message adopter. Their repo is theirs to change.

**2. The launcher goes to the harness.** `launch.rs` (555) + `tmux.rs` (226) add role prose, a
deny list, four env vars, a worker name and a detached start. Claude Code ships `-w/--worktree`,
`--tmux`, `--agent`, `--disallowed-tools` and `env`. **Path**: `.claude/agents/worker.md` plus
deny/env in settings; launch as `claude -w <name> --tmux --agent worker`. Keep the detached
path, which has no first-party equivalent. A probe proves a launched worker still gets the deny
list before the module is deleted.

**3. The channel's poll thread goes.** It re-reads the ledger on a timer to deliver about 45
pushes a day and writes ~3 MB of events doing it. **Path**: keep the conditions and
`air status --attention` as the query; replace the timer with `SendMessage`'s
`notify_when_idle` and `Monitor`. After item 6, since the poll is what inflates `air audit`.

**4. `cross-project-fence` — DELETE NOW.** A PreToolUse denial of a `tmux` command naming
another project's session. Zero firings ever. The owner shortened its removal condition rather
than waiting for November. **Path**: delete the check and its probe, record the deletion and the
condition it met.

**5. The commodity sweep.** Anything left whose job is spawn, isolate, watch. **Path**: one
`do-less` pass after 2 and 3 land, not before.

## 2. Surfaces that report something untrue

**6. `air audit` calls evaluations "fires".** 10,722 log lines against 45 actual pushes; alpha's
audit says this, not `review-waiting`, is the real deletion candidate. **Path**: two fixes — a
pushes column derived from `hook_emissions`, and stop `status::record_and_log` writing the whole
condition set every tick. Probe: a condition holding for an hour writes one row, not 1,728.

**7. `air doctor` shows 7 of 11 tables**, and the MCP server claims three prompts it does not
serve. `hook_emissions`, `conditions`, `lease_wants` and `bd_cache` are invisible, which is how
zero-leases nearly went unnoticed. **Path**: enumerate from `sqlite_master` so a table added
tomorrow appears tomorrow; delete the prompts comment. Currency, not presence.

**8. `.air/events/` grows ~3 MB per active day and nothing collects it** — and it is the only
artefact that caught the audit's own errors, so it cannot simply be truncated. **Path**: `air gc`
with a stated retention, manual first. After item 6, then re-measure before choosing a window.

## 3. Alarms that fire on correct behaviour

All four are adopter's, and they are one shape: a condition computed from the absence of
events, when correct work is often eventless.

**9. `air handover` trips its own alarm.** It is documented as the way to find what is missing,
and every run increments the counter and pushes at the coordinator. **Path**: count only the
hook path toward the condition; the gate already knows which invoked it.

**10. `idle-without-claim` fires during a correct close.** It fired on a worker writing a close
reason and a digest. Correct work has an eventless tail (digest, close) and an eventless head
(reading a bead's citations before claiming). Air's own snapshot said `working` in the same pass.
**Path**: not a longer threshold. Consult the session's own last-seen state rather than the
absence of git and bd events, and suppress while a claim is held and the session is live.

**11. `handover-not-green` repeats.** Eleven lines for one worker, all the HEAD-past-green
pattern: the digest commit moves HEAD past the recorded green, so every hand-over costs a second
full verify. **Path**: backlog item 1 — carry a green forward when `git diff --name-only G..HEAD`
touches only repo-declared verify-irrelevant paths. Until then, one line per worker with a count.

**12. Claim rows survive `bd close`**, so alarms keep firing on closed, landed beads — three
repeats of one alert in their round. **Path**: backlog item 4. The PreToolUse gate already sees
the close; a PostToolUse on success releases the claim with reason `closed`.

## 4. The landing window

adopter's largest single cost, and Air has no answer today.

**13. Every land invalidates any verify in flight**, because the gate wants a green at a HEAD
containing main. A full verify is ~420s and the landing rate is faster, so no cadence works.
Their coordinator called it a signal Air does not have and ran a hand protocol instead: a worker
warns before a verify that matters, the coordinator holds. **Path**: `air record verify` already
knows a verify started. Publish it — `air status` shows verifies in flight with elapsed time,
and `air land` warns or refuses while one runs. That turns a relayed fact into a lookup, which
is what Air is for.

**14. Nothing says whether a land is in flight.** The merge commit appears minutes before the
verify finishes with rollback armed, so the coordinator reported "landed" three times before the
process exited, then resorted to `pgrep` — which misled them twice, because `pgrep` with no
match makes `ps` list every process the user owns. **Path**: write the landing row at merge time
as `in-flight`, update to `landed` or `rewound` at exit. `air status` reads it. Nobody greps a
process list for a fact the ledger holds.

**15. A red land owes a message to whoever merged main during the armed window.** A worker who
merged unverified main — the documented thing to do — keeps the rewound commits: a recorded
green for a tree main will never have, with `air handover` passing and `air land` merging it
back. **Path**: on a rewind, name every worktree whose HEAD contains the rewound sha. A
`merge-base --is-ancestor` loop over the worktree list Air already has.

**16. A worker's green is a green in a worktree; `air land` verifies in the main checkout.**
`.git` is a file in one and a directory in the other, and their probe passed everywhere except
the landing. **Path**: no mechanism. One sentence in roles.md, earned because the failure is
invisible from a worktree.

## 5. Latency and budgets

**17. bd's median is 1760 ms over 260,601 calls** on adopter's machine; ours is 1396 ms over 9.
Their `air claim` hit the 10s cutoff repeatedly under load with the write landing anyway, and
`air status` takes ~20s there, so their `reclaim.py` wraps it in a 60s timeout. **Path**: backlog
items 2 and 15 together — a distinct `timeout` decision value, one internal retry, a message
saying bd's state is unknown rather than asserting nothing was written, and a larger budget under
load. Design against their median, not ours.

**18. `air triage`'s batch mode — DELETE.** Serial bd calls under one 5-second budget put the
ceiling at about two ids; they triaged 34 captures as 34 invocations. **Path**: remove batch
mode, document one at a time. A batch that silently handles two of thirty-four is worse than none.

## 6. Measure, no rules

**19. Agent-to-agent traffic is invisible.** The PreToolUse matcher is `Edit|Write|MultiEdit|Bash`,
which feeds the edit journal, the peer warning and the gate. `SendMessage` is not in it, so one
worker sent ~46,900 characters in a day and the ledger says zero. **Path**: add `SendMessage` to
the matcher; record sender, recipient, byte count. Report per worker per round. No threshold.

**20. `peer-warning` has no measured effect.** 33 fires, no recorded collision cost prevented, no
recorded case of a worker acting on it. **Path**: record whether the warned worker then edited
the file anyway. That one field is the only honest route to a delete.

**21. Ten of fourteen mechanisms have never fired**, including `landed-not-closed` and `stuck`.
**Path**: print the never-fired set in `air audit` under its own heading, so it is a standing
question rather than something someone notices once.

## 7. Declined for now

**No orchestrator is adopted.** herdr (the fleet layer to rent), scion, tutti and loki-mode are
all declined: "our system works for now" (owner, 2026-08-29). The landscape's position 1 stands —
keep Air, delete its commodity half, keep the gate. herdr's `agent wait --until done` remains a
second answer to item 13 if that is ever revisited; the entry in `0007` §10.5 is the record.

The gate stays keyed to a bead id and a sha, which is why it would port to another harness
untouched. Worth a probe if anyone is ever tempted to key it to Air's session model.

## 8. How Air itself gets built

adopter's standards are adopted where they differ from ours.

**22. A probe must have been seen failing**, and the mutation must reach the code the probe
exercises. They found three ways a revert demonstration misleads: a mutant that does not compile;
a blanket mutant that only proves the probe is connected at all; and a mutation that hits a
neighbouring path and reports a good probe as vacuous — the costliest, because the response is to
go and "fix" a good probe. `air selftest` claims only that a probe matching nothing prints red.
**Path**: each probe records the mutation that made it red. Start with the gate's probes.

**23. Two questions for an acceptance clause at filing time**: what settles this — a command, a
file state, or a person? And if it were satisfied, would anything be different? Eleven of their
99 auto-closed beads had acceptance no rule could ever settle. **Path**: into the `decomposition`
skill and the `bd create --validate` guidance. This is where `air land`'s unreadable-clause
verdicts come from, and the fix is at filing, not at landing.

**24. A probe reads a rule's number from the rule.** Their log-cap probe hardcoded 45 against a
cap the owner had moved to 100, and failed on the rule being correct. **Path**: sweep our probes
for hardcoded thresholds — `anti-brittleness` pointed at ourselves.

**25. A project-diligence skill, loaded on every session in this repo.** Owner's proposal.
`air doctor` and `air audit` before trusting a number; the installed binary checked against the
repo (`strings $(which air)`, from the 2026-08-22 incident, and adopter's "a tool's behaviour
is only true where the tool is installed — on a branch it is a plan"); re-derive a number rather
than re-read it; confirm a probe has been seen failing. **Path**: build it before the next round,
not after. It is the only item here the owner asked for ahead of the rest.

## 9. Still owed, unscheduled

`do-less`'s six questions answered per mechanism, `anti-brittleness`'s failure-direction question
per parsing site, a sentence-by-sentence sweep of `roles.md` and `CLAUDE.md`, and the multi-agent
question itself. Alpha gathered the firing evidence and stopped, declining the last on one day's
data — three of that day's defects were each found by a *different* session than wrote the code,
against ~46,900 characters of coordination for one worker.

Items 19–21 are what make the multi-agent question answerable at all. Nothing here should be
attempted before that traffic is measured.
