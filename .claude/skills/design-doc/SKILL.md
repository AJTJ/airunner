---
name: design-doc
description: Write or update the one architecture document that describes a system as it is today (for Air, docs/design.md). Use when asked for "the design doc", "the architecture doc", "document the system", "what does the system look like", or whenever a change alters a component, an interface, a flow, an invariant, or the operational topology. Produces a doc in the standard design-doc shape (context, overview, components, interfaces, data, flows, invariants, failure model, operations, open questions) with a Mermaid diagram wherever a structure or a flow is described, every claim traceable to a file or a command, written in plain technical prose. For a proposal about a future shape use the system-design skill; for the why behind a decision use docs/decisions.md.
metadata:
  version: 1.0.0
---

# Design doc

A design doc describes a system as it is. It is the one document a new reader opens to learn
what the parts are, how they talk, what they promise, and where the edges are. Everything
else in `docs/` is a ruling (decisions), a rule shipped to adopters (rules), or a record of one
session (journal, digests); proposals are lines in this doc's TODO section and technology
choices are rows in its technology-decisions section. None of those is a substitute for it.

The shape is the ordinary industry one (a Google-style design doc, trimmed of the parts that
only make sense for a proposal). What this skill adds is three rules that keep it honest.

## Three rules

1. **Truth, dated.** The doc describes the code at a named commit and a named binary version,
   in its header. Every statement about behaviour points at where it is true: a source file, a
   command whose output shows it, or a table in the ledger. When the reader could reasonably
   ask "is that still so?", the pointer answers. Numbers come from a command run while writing,
   never from another document (`project-diligence`).
2. **A diagram wherever a structure or a flow is described.** Components get a component
   diagram, a request path gets a sequence diagram, a lifecycle gets a state diagram, the
   data model gets an entity diagram. Prose then explains what the diagram cannot show:
   ownership, guarantees, and why. `references/diagrams.md` says which Mermaid form fits
   which subject and how to keep one readable.
3. **Write normally.** Aim for four to six thousand words. Cut by moving history to
   `docs/decisions.md`, evidence to the references of the skill that uses it, and proposals
   to one or two lines each in the TODO section (there are no plan or research files, owner
   2026-09-25), never
   by compressing sentences into fragments or aphorisms. Plain technical prose, the way you
   would explain the system to a colleague who is going to maintain it. No house style, no
   clever phrasing; the owner asked for this explicitly on 2026-09-14 after a first draft
   read as stilted.

## The shape

Sections, in order. `references/template.md` has the skeleton with the questions each section
answers. Skip a section only when it has nothing to say, and say so in one line rather than
leaving a gap.

1. Header: what the system is in two sentences, the commit and version described, the date.
2. Context and goals: the problem, who uses it, what it deliberately does not do.
3. Overview: one diagram of the whole system and its neighbours, then a paragraph per box.
4. Components: one subsection each, with its responsibility, what it owns, what it depends
   on, and its size (files, lines) so the reader can judge weight.
5. Interfaces: every surface another party uses. For a CLI, the commands grouped by who runs
   them; for a server, the tools and resources; for hooks, the events and what each does;
   for files, their paths and owners; for environment variables, their meaning.
6. Data: the stores, their schemas at the level of tables and purposes, who writes each, and
   the retention rule.
7. Flows: the main paths through the system as sequence diagrams, one per flow, with the
   refusals and the facts recorded along the way.
8. Invariants and guarantees: what is always true, and what enforces it.
9. Failure model: what happens when each dependency is slow, absent, or wrong; the fail-open
   and fail-closed choices, stated as such.
10. Operations: how it is installed, launched, upgraded, and observed; what a release is.
11. TODO: the one explicit list of what is still to do, one line per item with the date it
    was added and where the detail lives. A line gets a bead id when it becomes a bead and is
    deleted when it lands. This replaces any separate TODO or backlog file (owner, 2026-09-14:
    "we work off of a singular design doc now").
12. Technology decisions: one row per choice (language, stores, dependencies, harness, the
    patterns borrowed), with why in one to three lines and a source, a URL with its access date
    or a `path:line`. This replaces a research directory (owner, 2026-09-25: "having a list of
    technology decisions and justifications is useful, but that's about it").
13. Glossary: one line per term the doc uses in a specific sense.

## Writing it

- Start from the code, not from the existing docs. Run the binary's `--help`, read the
  module headers, list the tables, print the launcher's argv. Then write.
- Name each thing exactly one way, and use that name everywhere (a "worktree" is never a
  "checkout"). Put the names in the glossary.
- Describe rather than argue. The doc says what is true ("main only moves by fast-forward
  onto a verified commit"); the reason belongs in `docs/decisions.md` with its date, and the
  doc links to it.
- Prefer a table for anything with more than three parallel items. Prefer prose for a line
  of reasoning. Never a bold-label bullet wall.
- Diagrams show the mechanism, not the org chart: an arrow means a call, a write, or a
  message, and the label says which.
- After writing, run `references/checklist.md`.

## Keeping it current

The doc is edited in the same change that alters what it describes. A change that adds a
command, a table, a hook event, a condition, or an environment variable, or that changes who
may do something, updates the matching section and the header's commit. In this repo a
release (`make release`) is the natural moment to re-read the whole thing against the tree.

## Provenance

Written 2026-09-14 at the owner's request: "one design doc that describes the architecture of
our system and follows a very idiomatic design doc for a system that is not too verbose and
that is very clear ... Include Mermaid diagrams where useful ... based on the truth as it is
right now ... a writing style skill that enforces clear, succinct sentences and prose
structure. No unnecessary terseness or shortness." Section order follows the common industry
design-doc shape (context, goals and non-goals, overview, detailed design, alternatives,
cross-cutting concerns), with the proposal-only parts removed and interfaces, data, flows,
invariants and failure model made explicit because they are what a reader of an operating
system needs. Diagram guidance draws on the `mermaid-diagrams` and `artifact-diagramming`
skills. Removed when the repo has no `docs/design.md` to keep, or when the doc has gone two
releases without being re-read against the tree, at which point the rule is not being kept
and should be dropped rather than pretended.
