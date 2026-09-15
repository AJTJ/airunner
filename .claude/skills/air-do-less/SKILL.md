---
name: air-do-less
description: The most important skill in this repo; invoke before ANY change, addition, or review of Air's mechanisms or shape (including whether the multi-agent pattern is still worth it). Use before adding any rule, hook, deny entry, attention condition, procedure, prompt, or role text to Air or a target repo, and when reviewing existing ones. Asks whether the addition removes a recorded failure or merely directs a capable model; requires a named removal condition; prefers measurement over enforcement and silence over advice. Trigger on "should we add", "add a check", "add a rule", "enforce", "require", "the agent should always", or when a mechanism is proposed without an incident behind it.
metadata:
  version: 1.0.0
---

# Do less

Air exists to make models productive, not to tell them how to work. Every constraint is a bet
that the model cannot be trusted with something; that bet decays as models improve, and a
constraint nobody re-examines becomes a throttle. Opinionated frameworks (Gas Town: roles,
molecules, convoys, merge slots) are the cautionary case: mechanisms ahead of measured need.

## Before adding anything, answer in writing

1. **Which recorded failure does this remove?** Cite the incident (capture id, retro line,
   decisions entry). "It might happen" is not a failure. No incident, no mechanism.
2. **Is it the model's judgement or a fact the model lacks?** Air supplies facts (green at
   sha, who holds a file, who is stuck). It does not supply judgement (what to work on, how to
   split, when to ask). If the proposal encodes judgement, stop: make it a measurement or a
   prompt the coordinator may invoke, not a rule it must obey.
3. **Can it be a measurement instead of a gate?** Count first; gate only after the count shows
   the cost. (WIP cap: rejected as a gate, kept as a count. Hand-over gate: advisory for a full
   round before `AIR_ENFORCE=1`.)
4. **Can it be silent?** A hook that speaks on the ok path, or repeats unchanged, is noise the
   human reads. Speak once per real change or not at all.
5. **What is the removal condition?** Write it next to the mechanism: "remove when a round of
   ledger data shows zero X", "remove when models stop doing Y", "remove when bd provides Z".
   A mechanism without one is not accepted.
6. **What is the smallest version?** A pattern over an enumeration; a flag over a file; a count
   over a queue; one check over a state machine; nothing over prose.

## Nothing is sacred

The questions above apply to Air's own shape, not only to additions. The multi-agent pattern
(coordinator plus workers in worktrees), beads, the hooks, the channel, Air itself: each is a
bet about what a model cannot do alone at the time it was made. Re-ask, with the ledger and the
current model in front of you: would one capable session with the facts do this better? If yes,
the answer is to remove the structure, not to refine it.

## Reviewing what exists

Quarterly, or after any round: for each hook, deny rule, attention condition, and roles.md
paragraph, ask 1 and 5 again with the ledger open. Anything whose failure has not recurred in
the record is a candidate for removal. Log removals in `docs/decisions.md` like additions.

**Before a measurement is used to justify removing a mechanism, check it against the raw
record for that mechanism.** A derived statement reads exactly like an observed one, and the
derivation is invisible at the point of use — the plausible number is the dangerous one, so
grepping does not catch this. Twice on 2026-08-22: four removal conditions were derived from a
bead's framing of an incident and read as recorded until the code was opened (air-zyo,
air-0y9); and `air audit` reported "peer-warning: 29 fires, 28 repeats, 1 subject", which was
its own subject counting rather than an observation — the raw log had 29 fires over 29 distinct
subjects and zero repeats, and the mechanism proposed for deletion was working exactly as
documented (air-s7c).

## Phrasing in roles and prompts

Say what is true and what is available; avoid "always", "never", "must" unless a refusal
backs it. Prefer "here is the fact" to "do this". The model reads the facts; the machinery
holds the one refusal.

## Provenance

Owner rule 2026-08-21 (`CLAUDE.md` Rules, "Do less"); Gas Town cautionary note
(`docs/decisions.md` 2026-08-18); corpus principle "no LLM middle-manager"
(`docs/research/evidence.md`); research commissioned the same day
(`docs/research/evidence.md`, when written). The raw-record check under
"Reviewing what exists" is air-21c (owner, 2026-08-22), from two cases in one round; removed
when `air audit` derives nothing and every number it prints is a direct count of recorded
events, at which point there is nothing to check against.
