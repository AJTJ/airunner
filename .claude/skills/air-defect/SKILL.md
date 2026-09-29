---
name: air-defect
description: Use every time a defect, a finding, a red run, a refusal or a capture is explained, or a fix is proposed, whether asked or not, by the coordinator to the owner or on a bead. Trigger on "explain this defect", "what went wrong", "why did X happen", "propose a fix", and on any capture triaged to a bead. Writes the explanation in seven parts, from the ground up, with the fix backed by the state model and the register of solved issues.
metadata:
  version: 1.0.0
---

# Defect

A defect is explained the same way every time, so the owner can read it without the context the
writer has. Write seven parts, in this order. Each is a short paragraph in plain words. Define
a term the reader may not have in one line, the first time it appears.

## The seven parts

1. **What the parts do.** Only the parts of Air this defect touches, one line each.
2. **What happened.** In order: who did what, and what was expected instead.
3. **Why.** The rule or code that produced it, named by file, function or bead, in one sentence.
4. **The model.** Which state or transition of the lifecycle table (`.air/docs/design.md` §6) this
   is; whether it is a stuck state, a loop that wastes work, or neither; and why the model test
   (`crates/cli/src/cmd/lifecycle.rs` in Air's repository) did or did not flag it.
5. **Precedent.** The entries in `.air/docs/solved-issues.md` that touch the same mechanism,
   quoted by title, with their "Do not" lines, and whether the fix reuses a rule that already
   exists.
6. **Fix and cost.** The change in one sentence, what it must not regress, and the cost of leaving
   it, as a number with its source.
7. **What the fix could break.** Walk the fix through the lifecycle table (every transition it
   touches, and the ones beside them), the "Do not" lines for the same mechanism, and the model
   test, run with the change: report any stuck state or waste it introduces. Name each way the
   change could create a new defect, with the evidence that it does not or the reason it is
   accepted. A fix without this part is not proposed.

## Rules

- No wall of text: seven paragraphs, each under 80 words, is the shape.
- Every number has its source (a command, a ledger count, a log, a bead).
- Adopters are "an adopter" (CLAUDE.md).
- Parts 4 to 7 become the bead's `## Precedent` section, which the precedent-before-change step
  requires (air-fd8p).

## Worked example

Trial defect 1, 2026-09-29: the re-offered red member.

**What the parts do.** Workers finish on separate branches. The lane takes several branches at
once, merges them onto main together, and runs the tests once. Green: they all land. Red: nothing
lands, and the lane has to work out which branch broke it (splitting).

**What happened.** A batch of two went red: worker-3's change was broken on purpose (the trial's
red-batch scenario), worker-2's was fine. The lane split by hand: merged worker-2's branch alone,
green, landed it. Air then printed its "ready to cut next" list, which held worker-3's same broken
commit. The lane cut it and it went red again: a test run wasted on a commit Air already knew was
red. Only after worker-3 committed a revert did things move.

**Why.** air-hyli holds a red commit out of the next cut only when it was the only member of the
red batch (status.rs red_alone). With two or more members Air does not know which is guilty, so it
holds nobody out.

**The model.** Not a stuck state: the work can still land. A loop that wastes a verify each time
round, which the model does not flag; it finds deadlocks, not waste.

**Precedent.** "A red batch report silently stopped after 20 further runs" (air-cyf): a standing
red is repeated until a later green carries each member, no window. "A branch whose red batch had
it as the only member stayed batch-ready" (air-hyli): do not decide red-at-head from the batch sha
alone. The readiness rule can reuse cyf's test.

**Fix and cost.** After a red batch each member stays out until it commits past that point or a
later green carries it; the innocent member returns when the split proves it green, the guilty one
waits for a commit. Must not regress: a member cleared by a later green is offered again. Cost of
leaving it: one wasted verify per multi-member red (minutes on a Rust repo, more under load); an
adopter hit it twice in one day.

**What the fix could break.** The re-offer fix could hold an innocent member out forever if the
split never records a green carrying it: a hand split writes no batch_cuts row (trial omission 3).
So the fix must count any recorded green containing the member's sha, cut or not, and the model
test gets the red-then-split ordering.

## Where it reads

`air install --write` writes `.air/docs/design.md` and `.air/docs/solved-issues.md` from the
copies embedded in the binary, so parts 4 and 5 read the documents the running Air was built
with (air-g9dx). In Air's own repository they are `docs/design.md` and `docs/solved-issues.md`,
and `.air/docs/` holds the same text once installed.

## Provenance

Written here on 2026-09-29 (air-zese), at the owner's request after a defect was explained badly
twice: "explain the scenario succinctly, from the ground up in simple language, providing the
context I need, and then a solution backed up by the state model and historical precedence." The
owner added the same day that it is used for every potential defect, every time, that part 4 is
the state model, and the seventh part. The worked example is the coordinator's, from the trial of
2026-09-29. Removed when defects on beads and to the owner are written in this shape without the
skill being invoked, or when the lifecycle table or the register is retired.
