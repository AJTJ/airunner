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

### Every other hook · `air hook`

`PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`, `PermissionDenied`,
`SessionStart`, `SessionEnd`, `Stop` and `SubagentStop` each run `air hook`, which dispatches
the event, fails open, and writes one event line per invocation (`CLAUDE.md`, systems index;
`docs/design.md`). This repo is its own adopter, so these are what `air install` writes.

## Deliberately not ported

- **`SessionStart` / `PreCompact` → `bd prime`** (in another-project' settings, and an adopter's
  uses `bd prime --hook-json`). Not wired; add it on a named pain.
- **Any `permissions.allow` list.** None needed for the current work; add per named pain.
