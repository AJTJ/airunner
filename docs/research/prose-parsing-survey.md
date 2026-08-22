# Survey: where Air infers structure from prose

Owner, 2026-08-22, watching `air land`'s selection being built: *"parsing just open prose is
kind of brittle… if we are going to start inferring task IDs from somewhere, then they should
probably be in JSON format or something that is useful."* Filed as air-4re.

This is the survey the bead asks for: every place Air reads structure out of text a person
wrote freely. For each, what it parses, what breaks it, and whether a structured source exists
or is cheap.

## The thing the survey found

The four sites were expected to differ in how fragile they are. They differ in something more
useful: **which direction they fail.**

| | fails toward | cost of a parse miss |
|---|---|---|
| Commit-message bead attribution | reporting **more or less** than the branch did | a person reads a wrong line |
| `## Acceptance Criteria` section | reporting **fewer** clauses | the close looks unchecked, or checked against nothing |
| Digest presence check | **passing** the gate | the hand-over gate is satisfied by a file that is not the digest |
| `is_handover_command` | **allowing** the write | the one refusal silently does not fire |

The bottom two fail toward *permitting*. That is not a property of how carefully they are
written; it is a property of where they sit. A parser that guards something should be counted
as absent until proven present, and neither of those is.

The top two are reports, and the bead's framing — "a wrong selection prints a bead that did not
belong, and a person reads it" — holds for them. It does not hold for the bottom two, and that
is the part worth carrying forward.

---

## 1. Commit-message bead attribution — `air land`

**Parses** every commit message in `main..<head>` for tokens shaped like `<lowercase>-<alnum>`
(`crates/cli/src/cmd/attribution.rs::prose_ids`, frozen).

**What breaks it.** Nothing syntactic — the shape is easy. The break is semantic and
unfixable by parsing: **a bead id appears in a commit message for two different reasons**,
because the commit did that bead, and because someone mentioned it ("builds on air-3pz",
"measured in air-869"). Mention and attribution have identical grammar.

air-7kp added an authorship filter and then a branch-point time bound to compensate. Measured
against one real branch, the count went **8 → 6 → 3**, and 3 was right. Each rule was a
statement about how a person happened to write a sentence, and there was no way to know the
pile was finished.

Two more that the rules did not address: a bead worked in a commit that never names it is
invisible, and a `Bead:`-less merge commit inherits nothing.

**Structured source: yes, and cheap.** A `Bead: <id>` git trailer, repeatable, written by
whoever makes the commit — `air claim` already knows the id. **Done in air-4re**: trailers are
read first, and the prose scan survives only for commits before `FALLBACK_BEFORE`
(2026-08-23), dated so it can be deleted rather than argued about.

**And the narrowing had to be unwound with it.** The first cut kept air-7kp's authorship filter
and branch-point bound over *everything*, declared ids included. That is the same mistake one
level up — a heuristic confirming a fact a machine already wrote — and it bit within the hour:
merging `main` moves the branch point forward, so a bead claimed before the merge fell outside
its own bound and this bead's branch reported nothing. Declared and guessed are now kept apart,
and only the guessed half is narrowed. **Scaffolding for a guess must come down when the guess
does**, or it silently starts filtering facts.

## 2. `## Acceptance Criteria` section — `air land`'s report

**Parses** a bead description: a heading line whose text case-insensitively equals "acceptance
criteria" opens a region; inside it, lines starting `- ` or `• ` are clauses and other
non-empty lines continue the previous one (`crates/cli/src/cmd/acceptance.rs`).

**What breaks it.** A different heading ("Acceptance", "Done when", "Success Criteria" — which
bd's own template uses for epics); a numbered list; a `*` bullet; a nested heading, which
silently *closes* the region because any `#` line reassigns `inside`; a clause split across a
blank line, which is dropped rather than continued.

**Structured source: it already exists, and this parse is the fallback.** bd has
`--acceptance`, and `Issue.acceptance_criteria` reads it. The section parse exists only because
beads filed with `-d` alone put the criteria in the description — this repo is section-only (0
of 33 carry the field), adopter is field-mostly (647 of 711).

So the fix here is **not a better parser**. It is filing discipline: `bd create --acceptance`.
Until that holds everywhere, the parse is load-bearing for exactly the beads that skipped the
field. Worth noting that this one is *self-correcting*: every bead filed properly is one the
parser never sees.

## 3. Digest presence — the hand-over gate

**Parses** a directory listing: any `*.md` whose filename *contains the worker name* and whose
mtime is newer than the claim (`crates/cli/src/cmd/handover.rs::digest_newer_than`).

**What breaks it, and it fails toward passing.**

- The check never looks at the **bead**. A digest for a different bead by the same worker
  satisfies the gate for this one.
- Substring matching on the worker name: a worker called `alpha` is matched by
  `…-alpha2-….md`.
- mtime, not content: `touch` on any old digest satisfies it. So does an editor writing a
  swapfile-adjacent `.md`, or a rename.
- A worker whose name does not appear in its own digest filename fails the gate while having
  written the digest.

**Structured source: yes, and Air already holds the data.** `air record` and `air claim` know
the worker and the bead at the moment the digest is written. The gate could check a ledger row
("digest registered for bead X"), or read a front-matter `bead:` field, either of which names
the bead instead of guessing from a filename. **Not done here** — it is a change to the gate,
and the bead's last clause forbids patching prose parsers as part of this work. Filed as its
own item.

## 4. `is_handover_command` — the one refusal

**Parses** a shell command string: splits on whitespace, finds `bd` occurring as the first
token or immediately after `&&`, `;`, `||`, `|`, `(`, `{`, then matches `close`, or `update`
with `-s`/`--status` equal to `closed`/`awaiting_review`
(`crates/cli/src/cmd/hook.rs::is_handover_command`).

**What breaks it, and every one of these fails toward allowing.** A missed match falls through
to `Allow`, so the gate simply does not fire:

- quoting — `bd "close" air-1`, `bd close 'air-1'`
- an env prefix — `BEADS_ACTOR=x bd close air-1`
- a path or wrapper — `/usr/local/bin/bd close`, `command bd close`, `bash -c "bd close air-1"`
- newline-separated commands rather than `;` or `&&`
- `-s=closed` (the `--status=closed` glued form is handled; the short glued form is not)
- substitution — `` `bd close air-1` ``, `$(bd close air-1)`

**Structured source: no, and this is the honest answer.** The hook receives a shell string
because that is what the tool call contains; there is no structured field naming the command's
program and arguments. Air cannot fix this by reading somewhere better.

What follows from that is not "parse harder". It is:

- **Count the misses.** The gate's own removal condition is already "a full round with zero
  `handover-not-green` events", and `air audit` counts its firings. A gate that never fires is
  either unnecessary or broken, and today those two look identical.
- **Do not add cleverness that cannot be verified.** Each new pattern above is a guess about
  what someone will type, and the ones that matter are the ones nobody thought of.
- The real backstop is elsewhere: closing without a green also leaves a bead closed against a
  head with no recorded verify, which `air land`'s acceptance report and the landings row
  surface after the fact. Defence in depth beats a better tokeniser.

---

## Also prose, found while surveying

- **`land_command` / the landing report** builds `air land <bead>` strings that a person is
  expected to copy. Not parsed back, so it is output rather than inference — no risk.
- **`clauses_of` on an empty field** returns the whole block as one clause when it contains no
  bullets, so `--acceptance "criterion A"` is not read as "states none". That is a deliberate
  and documented rule, not drift, but it means a multi-sentence field collapses to one clause.
- **`air audit`'s `BOOKKEEPING` list** classifies event decisions by string. It is a list of
  known values rather than a parse of free text, and its failure direction is *loud* by
  construction (air-0y9 inverted it so an unclassified word shows up as a defect). Included
  here as the counter-example: the same shape, arranged to fail toward being noticed.

## What this survey is for

The anti-brittleness skill (`.claude/skills/anti-brittleness/`) asks the question this survey
answers site by site: *what does this mechanism depend on that a person chose freely, and could
a structured field carry it instead?* Site 1 is its worked example.

The removal condition on the whole pass, per the bead: **remove it when Air reads no structure
out of free text.** Sites 2 and 4 say that day is not close — one waits on filing discipline in
two repos, and the other has no structured source at all.
