---
name: user-summary
description: The shape of a reply that asks the owner to decide something or names a roadblock only they can clear: one line of state, then per item three bold labels, Question, Context, Recommendation, none over three sentences, the settling action last. Load before a message that asks the owner to choose, for "what are my decisions", and for a blocker that needs the owner. Do not load for a status, an answer to a question, a set of commands to run, or a report that nothing is needed; those are plain prose in the plain-language voice.
metadata:
  version: 1.1.0
---

# User summary

This shape is for a message that asks the owner to decide something, or that names a
roadblock only they can clear. It is not for a status, an answer to a question, a set of
commands to run, or a report that nothing is needed; those are plain sentences in the
`plain-language` voice, with no labels. Owner, 2026-11-05, after a reply that put two facts
and two shell lines under the labels: "this is really weird that you are using this
/user-summary skill for this sort of thing. I think it should only be used when there are
technical roadblocks that require a decision imho."

When it does apply, the owner reads a few lines and decides. A reply that is glib wastes the
decision; one that is long hides it. This skill is the shape between the two. It builds on
the `plain-language` skill, which still applies in full and is not restated here.

## The shape

One line of state first: done, blocked, a number moved, a choice is needed. Then each item
has three bold labels, each on its own line with a blank line between them:

```
1. **Question:** <the decision in a few words>

   **Context:** <what the thing is, what happened, why it matters; enough to decide without opening a file>

   **Recommendation:** <the thing to do, its one reason, its cost; and the one action that settles it, with the id or command>
```

A blank line between items. No part longer than three sentences. Context names the
mechanism, the number, the document sentence or the person the decision rests on, and says
what a bead, a spike or a question is in a clause the first time. The action that settles it
(a comment, a word, a command) is the last sentence of the recommendation, not a label of
its own.

A single explanation with no decision in it is prose, not an item: there is nothing to
recommend, so the labels would carry facts the owner only has to read.

## Simple is not glib

- A one-sentence answer with no context is glib; context with no answer is a wall.
- A technical term stays when it is the real name of the thing. Define it in a clause the
  first time if the owner has not used it in this conversation.
- A number goes in only when it changes what the owner does, and then with its unit and
  what it is compared against.
- Say "a decision, not a measurement" when it is one, and "inference" when the
  recommendation rests on reasoning rather than a result.
- No bead ids, file paths or commands in Context. They go in Recommendation, where the
  owner acts on them, and in a code block if they are to be copied.

## What to leave out

- Options the recommendation does not pursue, unless the owner asked for options.
- Anything the owner already decided in the conversation.
- Narration, restatement and closing offers, which `plain-language` already forbids.

## Worked example

Asked "how much of a technical lift would it be to give each harness its own pod from what
we have set up currently?", on 2026-11-05, with the lane verifying this repo on one dev pod
(before workers were allowed `air run`, air-nyi; kept for its shape, not its facts):

> Small on Air's side, awkward on the cluster's side.
>
> 1. **Question:** a machine per session: how big a change in Air?
>
>    **Context:** Air already keys targets by session, so the lane, each worker and the coordinator can each have one entry in the config and no code changes. The cheapest form is not separate pods but one pod with a different path per session, since each session's commit is pushed to its own clone there.
>
>    **Recommendation:** start with one pod and per-session paths, which is config only, today. Say go and the entries are filed.
>
> 2. **Question:** separate pods per session?
>
>    **Context:** the lift is in the cluster's deploy workflow, not in Air: a pod is named per user and base image, so one user gets one pod per image, all sharing one home volume. Several pods of one image need a name-suffix input there and a matching change to the job that deletes a user's pods.
>
>    **Recommendation:** a short change in a shared repo, so it is your call and your review; do it only when two sessions contend for the one pod.
>
> 3. **Question:** can a worker use a pod at all?
>
>    **Context:** only the lane can reach a pod from a launched session, because auto mode's classifier refuses a worker's `air run`; in Air the lane is the only session that verifies anyway.
>
>    **Recommendation:** allow the two exact lines on the worker launcher before any worker gets a pod, and name which sessions need to run things there. Say which, or none.

The reply as first given was three paragraphs with the recommendation at the end of the
third; in this shape each item ends with what settles it.

## Provenance

Ported 2026-11-05 (air-a0i) from the adopter's `user-summary` skill, at the owner's request:
"steal the user-summary skill ... and ensure you use it". The adopter wrote it on 2026-11-04
after a day of status replies that were too long or assumed the owner carried the ids in
their head; a one-paragraph-per-item shape and a five-label shape were rejected the same
evening, and three spaced bold labels were kept. Adapted here: its writing skill became
`plain-language`, whose rule on bold names these labels as the exception; its worked example,
which named the adopter's systems, was replaced with one from this repo; the lines that
restated voice rules were cut. Not shipped by `air install`: ship it when an adopter asks for
the shape. Narrowed 2026-11-05 (air-env), the first day of use: the shape applies to a
decision or a roadblock only, and a status, an answer or a single explanation is prose,
after the owner saw two facts and two shell lines under the labels and called it weird.
Removed when the owner says a plain reply is enough again.
