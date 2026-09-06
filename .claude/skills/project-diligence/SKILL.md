---
name: project-diligence
description: Use before stating any number, rate, or count about this repo (how often something fired, how many rows, how many probes, how long something takes); before claiming a mechanism fires, is enforced, or is shipped; and before saying what the installed `air` does. Answers "is this true, and where is it true?" - re-derive rather than re-read, check the installed binary against the repo, and confirm a probe has been seen failing. Trigger on "it fires N times", "there are N", "the gate refuses", "that is fixed", "already landed", "the hook catches", or any statement about Air's behaviour that was not produced by a command run just now.
metadata:
  version: 1.0.0
---

# Project diligence

Air keeps producing confidently wrong numbers, and every instance is the same shape: **a derived
statement in the grammar of an observed one.** The derivation leaves no trace in the sentence, so
grepping does not find it and hedging does not prevent it. The record: `air audit` reporting poll
re-evaluations as "fires" (air-21c); a surface claiming three MCP prompts it does not serve
(0007 §11); a fence reported as fixed that was committed on a worker branch and absent from the
binary the fleet ran (2026-08-22 round log, error 6).

## Before you state it

**Re-derive; do not re-read.** 0007 §11: *"re-reading the document finds nothing; re-running the
commands finds errors."* Three inventory errors survived every re-read of that doc and died the
moment the code was opened. A number in a doc, a bead, or a message is a copy; the source is the
command.

**Run the command that owns the number.** `air doctor` for rows, tables, schema and the pinned
`bd`. `air audit` for mechanisms, but read what it counts: ledger evaluations, while the channel
pushes only on change, so its "fires" is not "times a human saw this". The record holds two
different totals for that one incident because they cover different windows, which is the
argument for deriving the number now rather than quoting either.

**Check the binary against the repo.** A tool's behaviour is only true where the tool is
installed; on a branch it is a plan.

    strings $(which air) | grep -c "<a phrase from the change>"

Zero means the fleet is not running your change, however green it is. `make verify` runs
`air selftest` against *this tree's* build (`docs/decisions.md`, "what green means here"), a
different question from what `$(which air)` does.

**A probe is evidence only if it has been seen failing**, with the rule it names neutralised and
the mutation reaching the code the probe exercises (`docs/decisions.md`, "the post-audit
rulings", adopted from the adopter). "There is a probe for that" is not proof.

**Say where it is true.** Closed with proof, landed on main, and in the installed binary are
three different states. Name the one you mean.

**A true quote does not make the story around it true.** 2026-08-29, an hour after this skill
was written, its author reported a red build to the coordinator with a confident cause: the test
assertion named a real mechanism, a plausible story attached itself to it ("that change landed
without its tests"), and the story went out in the same grammar as the quote. The real cause was
a constant that had aged past a hard-coded date. `git show --stat` on the commit under suspicion
would have cost one command. When you catch yourself explaining evidence rather than reading
more of it, that is the moment.

## What this owns, and what it does not

`check-resources` asks *does this already exist* before you build. `anti-brittleness` asks *what
will drift under this* while you build. This asks *is what I am about to say true, and where*. It
applies at the moment of speaking or writing, including in a message to another session or
project, and most of all when the claim is one you already agree with.

## Removal condition

Remove when a full round produces no correction of a stated number or a stated behaviour: every
figure `air audit` prints a direct count of recorded events (air-21c's own condition), and no
repo-versus-binary correction in the round log.

## Provenance

Owner, 2026-08-29, plan [`0008`](../../../docs/plans/0008-consolidated-changes.md) item 25, the
one item asked for ahead of the rest. Incidents: air-21c and air-ha8 (2026-08-22);
[`0007-surface-audit.md`](../../../docs/plans/0007-surface-audit.md) §11; the six-error list in
`docs/notes/rounds/2026-08-22-air/2026-08-22-round-log.md`. The binary-versus-repo rule and the
seen-failing standard are the adopter's, adopted over ours by owner ruling (`docs/decisions.md`,
"the post-audit rulings"); the adopter found the fence instance by running `strings $(which air)`
on our behalf.
