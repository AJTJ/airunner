# docs

Reading material is the first table. Records nobody is asked to read are the second. There is
no plans or research directory (owner, 2026-09-25): proposals are TODO lines in `design.md`
§10, technology choices are rows in its §11, and facts an agent looks up at work time live in
the skill that uses them (`.claude/skills/*/references/`).

| Read when | Document |
|---|---|
| Learning what Air is and how it works today: components, every interface, the ledger, the main flows as diagrams, invariants, failure model, operations, the TODO list with the fleet's target shape (§10), and the technology decisions with their sources (§11) | [`design.md`](design.md) (kept current with the code; skill `design-doc`) |
| Integrating Air into a target repo: install, configure (`.claude/air.json`), upgrade | [`rules/adopting-air.md`](rules/adopting-air.md) |
| Which role a session is and what it may do; shipped to adopters as `.air/roles.md` | [`rules/roles.md`](rules/roles.md) |

| Record | Where |
|---|---|
| One file per session, appended as it goes; the round logs | `.air/journal/` in the main checkout (gitignored, not in git since air-1qnp) |

Adopter material (names, paths, copied files, the corpus behind the failure catalogue) lives
in `private/`, which is gitignored (air-bpj).
