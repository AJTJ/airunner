# docs

| Read when | Document |
|---|---|
| Wanting the "why" for the project | [`../README.md`](../README.md) · [`decisions.md`](decisions.md) (owner framing + dated decisions) |
| Understanding adopter's fleet as-built and its pain points | [`research/adopter-as-built.md`](research/adopter-as-built.md) |
| Which adopter rules are prose vs enforced; which skills to lift | [`research/adopter-enforcement-and-skills.md`](research/adopter-enforcement-and-skills.md) |
| Mining adopter's prior-art research (with original sources) | [`research/adopter-research-corpus.md`](research/adopter-research-corpus.md) |
| What metis does and what to borrow | [`research/metis-deep-dive.md`](research/metis-deep-dive.md) |
| Cutting a feature into epics and beads, exit criteria per phase, per-worker queues; the agile behind Metis (Flight Levels, Kanban, INVEST, story splitting, walking skeleton) and the adopter evidence for each rule | [`research/metis-decomposition-and-agile.md`](research/metis-decomposition-and-agile.md) |
| What exists in the wild (Rust first) — use vs borrow vs build | [`research/prior-art-landscape.md`](research/prior-art-landscape.md) |
| beads / Gas Town in depth (the coordination layer) | [`research/beads-and-gastown.md`](research/beads-and-gastown.md) |
| How a supervisor can drive/constrain Claude Code | [`research/claude-code-control-surfaces.md`](research/claude-code-control-surfaces.md) |
| Subscription vs API billing, mixed-backend cost arithmetic (primary sources) | [`research/claude-code-billing.md`](research/claude-code-billing.md) |
| What the live coordinator and workers say about the loop (interviews, verbatim) | [`research/coordinator-interview-2026-08-17.md`](research/coordinator-interview-2026-08-17.md) · [`research/worker-interviews-2026-08-17.md`](research/worker-interviews-2026-08-17.md) |
| The synthesis: do we need a runtime, and what shape | [`research/SYNTHESIS.md`](research/SYNTHESIS.md) |
| Building the first slice | [`plans/0001-first-slice.md`](plans/0001-first-slice.md) |
| Skills/reference material to port before building | a private skills inventory |
| Starting a session in a worktree: which checkout am I in, what may I do, how to hand over (and what Air enforces) | [`rules/worktree-protocol.md`](rules/worktree-protocol.md) |
| Which role am I (coordinator or worker), what I do, what I never do, how I hand over | [`rules/roles.md`](rules/roles.md) |
| What each role is allowed and required to do, and how far Claude Code can confine a worktree session to its role (settings, launch flags, `--agent`, hooks, native worktree isolation; verified against docs 2026-08-20) | [`research/agent-roles-and-confinement.md`](research/agent-roles-and-confinement.md) |
| Reading adopter's original research notes (verbatim copies, provenance-pinned) | [`research/adopter-notes/`](research/adopter-notes/PROVENANCE.md) |
| Checking whether a corpus claim survived independent source verification | [`research/verification/`](research/verification/) — 5 slices done; ticks in `verification/ticks/` with [`SUMMARY.md`](research/verification/ticks/SUMMARY.md) |
