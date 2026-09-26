---
name: plain-language
description: The voice rules for anything a person will read in this repo (README, docs, skills, commit and PR bodies, bead text, messages to the owner). Every other writing skill points here for voice. Invoke before writing or editing prose, and when the owner says "too verbose", "weird", "just speak normally", "plain language" or "simpler".
metadata:
  version: 2.0.0
---

# Plain language

This skill owns the voice for everything written in this repo. The `design-doc` skill says what
shape the design doc has; it does not restate these rules.

## The voice

Write the way you would explain something to a colleague who asked. Use complete, ordinary
sentences. Say each thing once, in plain words, and then move on.

1. **Complete sentences.** Every sentence has a subject and a verb. Do not use fragments for
   emphasis or rhythm ("One thing.", "Early, and honest about it."). When you remove an em dash
   or split a long sentence, check that both halves are still sentences.
2. **Common words.** Write "use", not "leverage". A code name is fine; a fancy synonym is not.
3. **Say it once.** No preamble, no summary of what you just said, no restating a point in
   different words for weight.
4. **Describe, do not perform.** State what the thing is and does. Do not describe your own
   writing or stance ("honest about it", "to be clear"), and do not dramatise ordinary facts
   ("the line in the sand", "the one refusal", "by design, and on purpose").
5. **No aphorisms.** Do not end a paragraph on a quotable line or a slogan ("A green belongs to
   a commit, not to a person's memory of one."). If the sentence carries a fact, write it as a
   plain statement. If it does not, delete it.
6. **No bold in running prose.** Bold is for a term a scanning reader needs to find, such as a
   list label in a reference table. Never bold a verdict or a whole sentence.
7. **One qualifier at most.** A sentence gets one hedge or one condition. If a claim needs
   several, split it or move the exceptions to the document that owns them.
8. **Citations out of the sentence.** Prose meant for people does not carry bead ids, dates,
   `path:line` pointers or "(owner, date)" in the middle of a sentence. Where a source trail is
   required (research claims, docs, commit bodies), put it at the end of the paragraph, in a
   table column, or in a Sources line. The rule to cite stays; only the placement changes.
9. **What first, why second.** Give the fact, then the reason in one sentence or a link.
10. **Cut what the reader would guess.** If a sentence could be deleted without the reader
    missing it, delete it.

## Length

Keep a bead description under 200 words and a message to the owner under 150. When a text is
too long, cut content. Do not shorten it by dropping grammar. The design doc's length is in
`design-doc`.

## Check before you ship

Read it aloud. A sentence that sounds like a slogan, a headline or a closing line gets
rewritten as a plain statement.

## Provenance

- Written 2026-08-22 for Air at the owner's request ("the README is way too verbose"). Rules
  follow plainlanguage.gov and the Plain English Campaign (short sentences, common words,
  reader-first order), from memory.
- 2026-09-25, v2.0.0 (air-k8bj): made the one owner of voice after the owner said the writing
  sounded strange and asked for normal speech. The 20-word cap, the "Say the thing. Stop." opener and
  the README and skill word budgets were removed because they pushed writing toward fragments
  and compressed prose. Rules 1 and 4 to 8 were added from the patterns the owner objected to.
- Removed when the owner stops reading prose written here, or when another skill owns voice
  and this one duplicates it.
