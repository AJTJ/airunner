# Minimal example

A three-file project after `air init --write`. It shows what a check is and what Air adds to a
project.

## The project

- `greet.sh` prints a greeting.
- `test.sh` is the project's tests. It exits 0 when every case passes.
- `Makefile` has one target, `verify`, which runs `test.sh`.

## The check

The check is the command that decides whether the project is in a good state. Here it is
`make verify`. A bead can only close, and a batch can only land, after the check passes at that
commit. Air runs whatever command you give it and records the result:

```sh
air record verify -- make verify
```

In a real project the check is usually your test suite, plus anything else you would want to
pass before shipping, such as a linter or a build.

## What Air added

| File | What it is |
|---|---|
| `CLAUDE.md` | A short stub. The fleet's rules live in `.air/roles.md`, so this only names the check and the project's own rules. |
| `.claude/air.json` | Air's settings for this project. Empty lists are fine; `leases` and `precheck` are added only if needed. |
| `.claude/settings.json` | Claude Code hooks that call `air hook` on every tool call. |
| `.mcp.json` | The channel that tells the coordinator when something needs attention. |
| `.worktreeinclude` | Untracked files a new worktree needs, such as `.env`. Empty here. |
| `.gitignore` | Ignores `.air/`, plus lines bd adds. |

Not in this folder, because they are generated per machine:

- `.air/` holds the ledger, `roles.md`, and session journals. It is gitignored.
- `.beads/` is the task store created by `bd init`.
- `.claude/skills/air-*` are two skills `air install` writes.

## Try it

```sh
cp -r examples/minimal /tmp/air-example && cd /tmp/air-example
air init --write
git add -A && git commit -m "adopt Air"
air coordinator
```
