---
name: air-do-less
description: The most important skill in this repo; invoke before ANY change, addition, or review of Air's mechanisms or shape (including whether the multi-agent pattern is still worth it). Use before adding any rule, hook, deny entry, attention condition, procedure, prompt, or role text to Air or a target repo, and when reviewing existing ones. Asks whether the addition removes a recorded failure or merely directs a capable model; requires a named removal condition; prefers measurement over enforcement and silence over advice. Trigger on "should we add", "add a check", "add a rule", "enforce", "require", "the agent should always", or when a mechanism is proposed without an incident behind it. Also trigger before DELETING a mechanism on a count of zero: a zero is evidence only when the mechanism's subject occurred and it stayed silent, never when the subject never happened or the input never arrived.
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
   A mechanism without one is not accepted. A condition of the form "remove when this stops
   firing" must also name **what would have to occur for it to fire**, or it cannot be settled:
   see "A zero is only evidence when the subject occurred".
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
the record is a candidate for removal — a candidate, not a verdict; see the next section, because
most of a round's deletion candidates turn out to be zeros that mean nothing. Log removals in
`docs/decisions.md` like additions.

### A zero is only evidence when the subject occurred

**Before deleting anything on a count of zero, say which of three things the zero is.** Only the
first is evidence about the mechanism, and they are indistinguishable in the number itself.

1. **The subject occurred and the mechanism stayed silent.** Evidence. Delete it, or fix it.
2. **The subject never occurred.** Not evidence. The mechanism is untested, not useless.
3. **The mechanism's input never arrives, so it could not have fired whatever happened.** Not
   evidence, and the most dangerous of the three: it reads as (1) from the count and as (2) from
   a glance. Two sub-cases, and they call for opposite actions:
   - **3a, the sensor is broken or miswired.** The subject may be occurring constantly. Fix the
     wiring; deleting the mechanism removes the detector instead.
   - **3b, the configuration suppresses the input.** The mechanism is *dormant*, not dead, and
     revives with no code change if the configuration changes. Neither delete nor repair: decide
     it on whether the thing is worth reporting at all, and record that the count cannot settle
     it.

Four instances in one day, 2026-08-29, all initially read as (1):

- **`air lease`.** Plan 0007 recommended deleting it on zero rows in this ledger. The adopter used
  it every round: a worker read `air lease status`, saw `runtime` held by a peer, and took
  different work rather than routing around it. Case (2). The owner reversed the verdict
  (air-uae).
- **`lease-held-by-dead-session` and `lease-stale`** (air-sze). Zero firings here because the
  `leases` table is empty here. Case (2), one level down, in the same week the owner reversed it
  one level up.
- **`landed-not-closed`.** Never fired in eight days; listed among the dead. Kept on the argument
  that six landings is not a sample — case (2). It fired twice within the hour and both times
  found a real defect in an acceptance clause.
- **`stuck`.** Offered as the counter-example, the zero that WAS evidence: a worker had wedged
  and it stayed silent. Checked twice, and it is neither (1) nor a broken sensor. Session state
  `stuck` is set only by the `PermissionRequest` hook (`cmd/hook.rs`), and
  `hook.PermissionRequest` appears **zero times in 39,071 event lines across eight days**. The
  hook is real and correctly registered; it never arrives because the fleet runs in **auto mode**
  (`permissions.defaultMode: auto`, `skipAutoPermissionPrompt: true`), so no permission prompt is
  ever shown and nothing is ever waiting on one — the `PermissionDenied` events say "Blocked by
  classifier", which is that classifier deciding instead of asking. Case **3b**: dormant, and it
  would work again the day auto mode is turned off.

That last one is why the question is asked of the **input**, not of the world. "Did a worker get
wedged?" and "did anything Air can see report a wedged worker?" have different answers, and only
the second is in the ledger. It also took two passes to land: the first check established that
the input never arrived and stopped there, which is enough to reject the zero as evidence but not
enough to say what to do about it. **Finding the input missing is half the answer; the other half
is why.**

The cheap version of the check: name the event, row, or field the mechanism reads, and count
**that** over the same window. A zero there settles it; a zero in the mechanism's own firings
does not.

**Prefer the table that holds the fact over the log that holds the narrative.** Counting `stuck`
in the event log is contaminated by the investigation itself: `grep` finds the word in capture
text and tool-call records, and returns 3 where the answer is 0. The `sessions` table cannot be
contaminated, because prose is never written to it:

    sqlite> SELECT state, count(*) FROM sessions GROUP BY state;
    idle|2   running|1   working|2        -- never `stuck`, in any row, ever

Same question, no parsing, no false positives. Where a count decides something, look for the
structured place the fact lives before reaching for the log. (ledger, 2026-08-29, arriving at the
same zero without having to be careful.)

**Before a measurement is used to justify removing a mechanism, check it against the raw
record for that mechanism.** A derived statement reads exactly like an observed one, and the
derivation is invisible at the point of use — the plausible number is the dangerous one, so
grepping does not catch this. Twice on 2026-08-22: four removal conditions were derived from a
bead's framing of an incident and read as recorded until the code was opened (air-zyo,
air-0y9); and `air audit` reported "peer-warning: 29 fires, 28 repeats, 1 subject", which was
its own subject counting rather than an observation — the raw log had 29 fires over 29 distinct
subjects and zero repeats, and the mechanism proposed for deletion was working exactly as
documented (air-s7c).

## If it changes what an adopting repo sees, it needs a notice — and a notice needs a release

Air is installed into other repos, so a mechanism you add or delete is a mechanism *they* get.
Question 7, after the six above: **does this change what a target repo sees, does, or may do?**
If yes it needs a row in `install::SURFACE`, and appending one **cuts a release**: a row in
`install::RELEASES`, the same version in `Cargo.toml`, `make release`. `make verify` fails until
they agree, so this is not something to remember — it is something to expect.

Two failures behind it, both from 2026-08-29. A round changed more than any before it and added
one notice, so an adopting repo would have learned almost none of it. And the surface version
that guards against a downgrade could move on nobody's authority, which fails toward
*permitting*: the refusal quietly stops noticing, and a stale binary shows a repo none of the
notices telling it to upgrade. **Removed when** Air is no longer installed into a repo it is not
built in.

## Phrasing in roles and prompts

Say what is true and what is available; avoid "always", "never", "must" unless a refusal
backs it. Prefer "here is the fact" to "do this". The model reads the facts; the machinery
holds the one refusal.

## Provenance

Owner rule 2026-08-21 (`CLAUDE.md` Rules, "Do less"); Gas Town cautionary note
(`docs/decisions.md` 2026-08-18); corpus principle "no LLM middle-manager"
(`references/evidence.md` beside the source of this skill in Air's repository); research commissioned the same day, the
guardrails-as-throttles findings F1 to F10 in that file. The raw-record check under
"Reviewing what exists" is air-21c (owner, 2026-08-22), from two cases in one round; removed
when `air audit` derives nothing and every number it prints is a direct count of recorded
events, at which point there is nothing to check against.

"A zero is only evidence when the subject occurred" is air-txa (2026-08-29), from four instances
in one day, three of which had already been decided wrongly and reversed. The third case — an
input that never arrives — came from checking the counter-example rather than quoting it:
`stuck` was offered as the zero that WAS evidence, and the ledger says its hook has never fired
at all. Its 3a/3b split came from air-byw, which asked WHY the input was missing and found auto
mode rather than a broken hook; the first pass had assumed a repair was needed. **Removed when** a round's removal proposals all name the subject they counted before
they name the firing count, at which point the question is being asked without the prompt.
