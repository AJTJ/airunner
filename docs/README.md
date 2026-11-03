# docs

Reading material is the first table. Records nobody is asked to read are the second. There is
no plans directory: proposals are TODO lines in `design.md` §10, technology choices are rows in
its §11, and facts an agent looks up at work time live in the skill that uses them
(`.claude/skills/*/references/`). `research/` holds the sourced output of research beads, one
file per bead, cited from a §11 row; nothing reads it at work time (retired 2026-09-25,
reopened for that purpose 2026-11-03).

| Read when | Document |
|---|---|
| Learning what Air is and how it works today: components, every interface, the ledger, the main flows as diagrams, invariants, failure model, operations, the TODO list with the fleet's target shape (§10), and the technology decisions with their sources (§11) | [`design.md`](design.md) (kept current with the code; skill `design-doc`) |
| Integrating Air into a target repo: install, configure (`.claude/air.json`), upgrade | [`rules/adopting-air.md`](rules/adopting-air.md) |
| Which role a session is and what it may do; shipped to adopters as `.air/roles.md` | [`rules/roles.md`](rules/roles.md) |
| Every problem Air has hit and solved, and what would regress it; read before changing a mechanism | [`solved-issues.md`](solved-issues.md) |

| Record | Where |
|---|---|
| One file per session, appended as it goes; the round logs | `.air/journal/` in the main checkout (gitignored, not in git since air-1qnp) |
| The sourced output of a research bead, each claim with its URL and access date or `path:line` | [`research/`](research/) |

Adopter material (names, paths, copied files, the corpus behind the failure catalogue) lives
in `private/`, which is gitignored (air-bpj).
