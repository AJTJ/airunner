# docs

Reading material is the first table. Records nobody is asked to read are the second.

| Read when | Document |
|---|---|
| Learning what Air is and how it works today: components, every interface, the ledger, the main flows as diagrams, invariants, failure model, operations, and the TODO list (§10) | [`design.md`](design.md) (kept current with the code; skill `design-doc`) |
| Wanting the why: every owner ruling, dated, append-only, with a standing-rulings index at the top | [`decisions.md`](decisions.md) |
| The one open proposal: a verification lane that lands and every role in a worktree, with the rulings still needed | [`plans/0009-fleet-system-design.md`](plans/0009-fleet-system-design.md) (skill `system-design`) |
| What bors, GitHub's queue, Zuul, Chromium's CQ, Uber's SubmitQueue and the agent-fleet queues do on a red, a conflict or a flake, and what transfers to a few branches on one machine | [`research/merge-queues-prior-art.md`](research/merge-queues-prior-art.md) |
| Whether Air is a lesser version of something that exists: the field, the overlaps, the health numbers, what was considered and not adopted, and how to refresh the roster | [`research/landscape.md`](research/landscape.md) |
| What Claude Code provides and can confine a session to (live inventory, dated), its hook events and their edge cases, and how to refresh the inventory | [`research/harness-facts.md`](research/harness-facts.md) |
| Working with `bd`: the surface Air uses, ready and claim semantics, the dependency guard, Gas Town's verdict, and whether to replace it | [`research/beads.md`](research/beads.md) |
| The findings the rules rest on: guardrails as throttles (F1 to F10), the decomposition evidence, the corpus principles, verified numbers and the claims not to repeat | [`research/evidence.md`](research/evidence.md) |
| Integrating Air into a target repo: install, coexist, what to change, upgrading | [`rules/adopting-air.md`](rules/adopting-air.md) |
| Which role a session is and what it may do; shipped to adopters as `.air/roles.md` | [`rules/roles.md`](rules/roles.md) |
| Starting a session in a worktree in this repo | [`rules/worktree-protocol.md`](rules/worktree-protocol.md) |

| Record | Where |
|---|---|
| One file per session, appended as it goes; the three round logs | [`journal/`](journal/README.md) |
| One digest per closed bead: what it did and its proof | [`digests/`](digests/) |

Adopter material (names, paths, copied files, the corpus behind the failure catalogue) lives
in `private/`, which is gitignored (air-bpj).
