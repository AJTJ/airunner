# Writing: two registers

> **Read when:** writing or editing anything under `docs/`, `CLAUDE.md`, or `.claude/skills/`.

Docs here are either **procedure** or **argument**. They want opposite things, and writing one in
the other's register is why documentation gets hard to read. The generic craft is in the
`writing-docs` skill (`.claude/skills/writing-docs/SKILL.md`); voice and banned patterns are in
the `writing-style` skill. This file is only the map of which bucket is which.

| Bucket | Register | Enforced |
|---|---|---|
| `docs/guides/`, `docs/reference/` (when they exist) | procedural | not yet (target: ≤ 30 words/sentence) |
| `CLAUDE.md` index rows, `air --help` text | procedural | not yet |
| `docs/rules/` | mixed: the imperative is procedural, the mechanism is argument | no |
| `docs/plans/`, `docs/research/` | explanatory | no |
| `docs/decisions.md` | explanatory (dated, append-only) | no |
| `.claude/skills/*/SKILL.md` | procedural (steps, tables) with short explanatory notes | no |

## The procedural rules

- One instruction per sentence. Split at every "and then".
- **One term per concept.** A "worktree" is never a "checkout" or a "clone". A "claim" is never a
  "lease" unless you mean the `bd` lease it wraps. A synonym reads as a different thing.
- Imperative for steps: "Run `cargo test`", not "the tests can then be run".
- Present tense, keep articles.
- Lists for sequences, tables for lookups.

## Why the cap stops at the procedural buckets

The adopter caps sentences at 30 words in `guides/` and `reference/` only (`make docs-check`), and
deliberately not in `plans/`, `rules/`, `notes/` or its log. Air has no such check yet; when one
is added (a candidate for an `air doctor` or hook check, since it is a prose rule that a
mechanical check can replace), it must keep the same boundary.

A length check pointed at a design doc does not produce a shorter argument. It produces a **missing
one**: the cheapest way to pass is to delete the qualification, the measurement, or the rejected
alternative, and those are the parts a future reader cannot reconstruct. Concision in the
explanatory register means no wasted words, never fewer claims.

30 rather than STE's 20-25: that standard targets aircraft maintenance procedures read by
non-native speakers under time pressure. 30 catches the sentences that have genuinely become
unreadable without fighting ordinary technical prose.

## Provenance

- Source: `the adopter's docs/rules/writing.md` (the adopter `f2ca891`).
- Ported 2026-08-18.
- Adaptations: bucket table retargeted to Air's `docs/` layout; `make docs-check` noted as not yet
  present here; domain example replaced with worktree/claim terms; em dashes removed.
