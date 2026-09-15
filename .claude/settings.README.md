# `.claude/settings.json` — what each hook does and where it came from

Project-scoped Claude Code settings for Air. Kept minimal on purpose: hooks are added only when
they remove a named pain (`CLAUDE.md` Rules). No permissions are granted here.

## Hooks

### `PostToolUse` · matcher `Edit|Write` · cargo fmt on `.rs` writes

**What it does.** After every `Edit` or `Write` tool call, reads the edited file's path from
the hook's stdin JSON (`.tool_input.file_path`). If the file ends in `.rs`, it walks up the
directory tree to the nearest `Cargo.toml` and runs `cargo fmt --all` there, then returns
`{"suppressOutput": true}` so the formatting is silent. For any other file it returns `{}`
(no-op). Failures of `cargo fmt` are swallowed (`2>/dev/null`) — the hook never blocks a write.

**Why.** Removes the "commit rejected / review noise because of formatting" relay: agents never
have to remember `cargo fmt`, and the pre-commit/CI check stays green. Requires `jq` and a Rust
toolchain with `rustfmt` on `PATH`.

**Provenance.** Copied verbatim from
`~/projects/another-project/.claude/settings.json` (another-project),
2026-08-18; identified in `.claude/skills/PROVENANCE.md` as the most directly relevant
hook config found across the source repos.

## Deliberately not ported (yet)

- **`SessionStart` / `PreCompact` → `bd prime`** (also in another-project' settings, and
  `the adopter's .claude/settings.json` uses `bd prime --hook-json`). Air has no beads store in this
  repo yet, so `bd prime` would fail or prime the wrong context. Re-add when `.beads/` exists.
- **Any `permissions.allow` list.** None needed for the current work; add per named pain.
- **Air's own hooks (`air hook …`).** Those belong to plan 0001 and will be wired here once the
  binary exists; this file is not the place to prototype them.
