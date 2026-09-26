# CLAUDE.md

This repo runs a small fleet with Air. The fleet's protocol (roles, how a bead goes from a claim
to main, proof, landing, and what Air refuses) is Air's and lives in `.air/roles.md`, appended
to every session by `air worker` / `air coordinator`. It is not restated here. Work is tracked
in beads (`bd ready`, `air claim`, `air capture`).

## What is this repo's own

- **Verify** is `make verify`, which runs `test.sh`. The lane records it with
  `air record verify -- make verify`.
- **Precheck**: none yet. If workers under a verification lane should run one, name it here
  and set `"precheck": true` in `.claude/air.json`.
- **Worktree setup**: untracked files a new worktree needs are listed in `.worktreeinclude`.
- **Shared resources** (a port, a simulator, Docker) are `"leases"` in `.claude/air.json`.

Domain rules for this codebase go below.
