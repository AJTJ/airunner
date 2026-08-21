# 0005 — Roadmap after the adopter migration

Status: decided 2026-08-21 (owner). Ordered. Each item is built against a named pain; the
first two are named here, the rest wait for round-one data.

## 1. Greenfield setup: `air init` (next)

The adopter adoption is a *migration*: existing rules, scripts, worktrees, and a cron to
retire, and it is the harder case. A new project with nothing in it should be one command.

`air init [--name <prefix>]` in an empty or fresh git repo: `git init` if needed (`main`),
`bd init` with the prefix and `--validate` defaults, `.gitignore` with `.air/`, `.claude/air.json`
with an empty deny list and `digest_dir` unset, `air install --write` (hooks + `.mcp.json` +
roles), a minimal `CLAUDE.md` stub that says only "roles: `.air/roles.md`; claim with `air
claim`; capture with `air capture`" and points at the adoption doc, and a printed next step:
`air coordinator`, `air worker <name>`. Ships with a probe (`init` on a temp dir, then
`selftest` and `status` succeed) and an integration test.

Input: the adopter adoption log (`adopter/docs/notes/air-adoption.md`), which records what
was tricky in the migration; whatever was tricky there must be absent from `init`.

## 2. Dogfood: build Air with Air

Run this repository the way adopter runs: `bd init` here, a coordinator session on main,
workers in worktrees, `air install --write` on this repo, beads for the remaining commands
(`next`, `peer`, `merge-advice`, `land`, `gc`, PreCompact re-inject, per-worker env in
`air.json`). Every pain Air's own development hits is recorded as a capture and becomes the
next bead. Success: one round of Air built by an Air-run fleet with the event log as the record.

## 3. After round-one data (adopter and dogfood)

In the order the ledger says, not this one: `air next` (route by shared state, overlap-ranked),
`air peer` / `merge-advice` (level-triggered behind-main), `air land` (land.sh
behaviour-for-behaviour, regenerate generated inputs, receipt, owns the generated-files list),
`gc`, PreCompact re-inject, `AIR_ENFORCE=1` for the hand-over gate once a round of advisory
data shows no false refusals, per-worker env and capabilities (`--with chrome`) in
`.claude/air.json`.
