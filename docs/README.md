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
| Whether Air is a lesser version of something that already exists: the harness layer, 186 orchestrators read one by one, what is commodity, what is contested, what is still ours, and where to look next (refresh monthly) | [`research/harness-and-orchestrator-landscape.md`](research/harness-and-orchestrator-landscape.md) |
| beads / Gas Town in depth (the coordination layer) | [`research/beads-and-gastown.md`](research/beads-and-gastown.md) |
| How a supervisor can drive/constrain Claude Code; §0 is the dated live inventory of what the harness already ships | [`research/claude-code-control-surfaces.md`](research/claude-code-control-surfaces.md) |
| The full inventory of everything Air ships (commands, ledger tables, hooks, MCP surface, integrations incl. beads, env vars, mechanisms, probes) with why each exists, what already does it, and a verdict | [`plans/0007-surface-audit.md`](plans/0007-surface-audit.md) |
| Subscription vs API billing, mixed-backend cost arithmetic (primary sources) | [`research/claude-code-billing.md`](research/claude-code-billing.md) |
| What the live coordinator and workers say about the loop (interviews, verbatim) | [`research/coordinator-interview-2026-08-17.md`](research/coordinator-interview-2026-08-17.md) · [`research/worker-interviews-2026-08-17.md`](research/worker-interviews-2026-08-17.md) |
| The synthesis: do we need a runtime, and what shape | [`research/SYNTHESIS.md`](research/SYNTHESIS.md) |
| Which metrics Air records, exactly how each is defined (the single list) | [`research/verification/ticks/2026-08-18-0430-measurement-spec.md`](research/verification/ticks/2026-08-18-0430-measurement-spec.md) |
| The first-round surface as built and how to operate it | [`plans/0004-first-round-surface.md`](plans/0004-first-round-surface.md) |
| What comes next, in order (greenfield `air init`, dogfooding, round-one-driven items) | [`plans/0005-roadmap.md`](plans/0005-roadmap.md) |
| Changes proposed from the adopter round and the do-less pass over them | [`plans/0006-post-round-changes.md`](plans/0006-post-round-changes.md) |
| Building the first slice | [`plans/0001-first-slice.md`](plans/0001-first-slice.md) |
| Running the work procedure: capture → triage → bead, decomposition, dispatch, hand-over, landing (one procedure; enforced vs judgement; metrics; rejected items) | [`plans/0002-what-to-work-on.md`](plans/0002-what-to-work-on.md) |
| Skills/reference material to port before building | a private skills inventory |
| The adopter adoption and first round, verbatim from the coordinator, with Air's synthesis | [`notes/rounds/2026-08-21-adopter/2026-08-21-adopter-round-verbatim.md`](notes/rounds/2026-08-21-adopter/2026-08-21-adopter-round-verbatim.md) |
| What the ledger can and cannot say about interruptions (adopter round, counts and gaps) | [`notes/rounds/2026-08-21-adopter/2026-08-21-interruption-counts.md`](notes/rounds/2026-08-21-adopter/2026-08-21-interruption-counts.md) |
| Air's backlog, incident-first, until Air runs beads on itself | [`notes/air-backlog.md`](notes/air-backlog.md) |
| Round records (one directory per round; verbatim, log, counts, boundary, positions) | [`notes/rounds/`](notes/rounds/2026-08-21-adopter/README.md) |
| Integrating Air into a target repo: install, rules to change, what Air now does, keeping it current | [`rules/adopting-air.md`](rules/adopting-air.md) |
| Starting a session in a worktree: which checkout am I in, what may I do, how to hand over (and what Air enforces) | [`rules/worktree-protocol.md`](rules/worktree-protocol.md) |
| Which role am I (coordinator or worker), what I do, what I never do, how I hand over | [`rules/roles.md`](rules/roles.md) |
| What each role is allowed and required to do, and how far Claude Code can confine a worktree session to its role (settings, launch flags, `--agent`, hooks, native worktree isolation; verified against docs 2026-08-20) | [`research/agent-roles-and-confinement.md`](research/agent-roles-and-confinement.md) |
| Reading adopter's original research notes (verbatim copies, provenance-pinned) | [`research/adopter-notes/`](research/adopter-notes/PROVENANCE.md) |
| Deciding whether a hook, deny rule, attention condition, roles.md paragraph, or gate check still pays its way as models improve (evidence, taxonomy, audit of every Air mechanism with removal conditions) | [`research/guardrails-as-throttles.md`](research/guardrails-as-throttles.md) |
| Checking whether a corpus claim survived independent source verification | [`research/verification/`](research/verification/) — 5 slices done; ticks in `verification/ticks/` with [`SUMMARY.md`](research/verification/ticks/SUMMARY.md) |
