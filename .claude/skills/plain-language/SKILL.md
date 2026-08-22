---
name: plain-language
description: Use before writing or editing anything a person will read (README, docs, summaries, bead text, messages to the owner). Plain, short language with measurable limits. Invoke when the owner says "too verbose", "plain language", "simpler", or a file is over budget.
metadata:
  version: 1.0.0
---

# Plain language

Say the thing. Stop.

## Rules

1. **One idea per sentence, under 20 words.** A second comma clause means split it.
2. **Common words.** "Use", not "leverage". A code name is fine; a fancy synonym is not.
3. **No preamble, no wrap-up.** Delete "In summary", "It is worth noting", "As mentioned".
4. **One hedge at most.** Two "probably"s is noise.
5. **What first, why second.** The reason is one sentence after the fact, or a link.
6. **Lists for three or more items.** One line each.
7. **Word budget.** README 400. Skill 300. Bead description 200. Message to the owner 150. Over budget means cut, not compress.
8. **Cut what the reader would guess.**

## Check before you ship

`wc -w`. Read it aloud. Split any sentence that needs a breath. Delete any sentence you would skip.

## With `writing-style`

That skill bans AI tells and em dashes. This one sets length. Both apply; shorter wins.

## Provenance

- Written 2026-08-22 for Air at the owner's request ("the README is way too verbose").
- Rules follow plainlanguage.gov and the Plain English Campaign (short sentences, common words, reader-first order), from memory. Word budgets are Air's own.
